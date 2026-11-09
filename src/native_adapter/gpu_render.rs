//! Headless native GPU frames: Blitz paints into a Vello hybrid (`vello_gpu`) scene on a
//! wgpu device and the frame is read back as compact RGBA8. The platform chooses the adapter
//! (wgpu default/environment selection); presentation, vsync and device-loss recovery stay
//! with the host.
//! 无头原生 GPU 帧：Blitz 绘制进 wgpu 设备上的 Vello hybrid（`vello_gpu`）场景，读回紧凑
//! RGBA8。适配器由平台选择（wgpu 默认/环境变量）；呈现、垂直同步与设备丢失恢复归宿主。
use anyrender::ResourceId;
use anyrender_vello_hybrid::{ImageManager, VelloHybridScenePainter};
use rustc_hash::FxHashMap;
use vello_common::paint::ImageId;
use vello_gpu::{
    ClearSettings, RenderSettings, RenderSize, RenderTargetConfig, Renderer, Resources, Scene,
    TargetInit, TextureBindings,
};
use wgpu_context::DeviceHandle;

/// Native GPU frame failure. Rejected covers owner/version/extent/budget admission, so no
/// GPU work was submitted; the other variants carry the device or renderer message.
/// 原生 GPU 帧失败。Rejected 表示归属/版本/尺寸/额度准入未通过，未提交任何 GPU 工作；
/// 其余变体携带设备或渲染器消息。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuFrameError {
    Rejected,
    NoAdapter(String),
    Device(String),
    Unsupported(String),
    Render(String),
    Readback(String),
}

/// Device-loss notification delivered by the host-owned wgpu device.
/// 宿主持有的 wgpu 设备发出的设备丢失通知。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuDeviceLost {
    pub reason: wgpu::DeviceLostReason,
    pub message: String,
}

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Target {
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
}

/// One wgpu device plus the Vello hybrid renderer state reused across frames and views.
/// 一个 wgpu 设备及跨帧、跨视图复用的 Vello hybrid 渲染器状态。
pub struct GpuRenderer {
    device: DeviceHandle,
    renderer: Option<(RenderKey, Renderer, Resources)>,
    target: Option<Target>,
    images: FxHashMap<u64, ImageId>,
    textures: FxHashMap<ResourceId, wgpu::TextureView>,
}

impl GpuRenderer {
    /// Use a host-owned wgpu instance, adapter, device and queue. This constructor performs no
    /// device acquisition; the host retains ownership of its window Surface and presentation.
    /// 使用宿主持有的 wgpu instance、adapter、device 与 queue；本构造不取得设备，窗口
    /// Surface 创建与呈现继续归宿主。
    pub fn from_wgpu(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        Self {
            device: DeviceHandle {
                instance,
                adapter,
                device,
                queue,
            },
            renderer: None,
            target: None,
            images: FxHashMap::default(),
            textures: FxHashMap::default(),
        }
    }
    /// Acquire an adapter without a surface (headless) and create its device.
    /// 不带呈现表面（无头）取得适配器并创建设备。
    pub fn new_headless() -> Result<Self, GpuFrameError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let device = pollster::block_on(DeviceHandle::new_from_compatible_surface(
            instance, None, None, None,
        ))
        .map_err(|error| match error {
            wgpu_context::WgpuContextError::RequestAdapterError(_) => {
                GpuFrameError::NoAdapter(error.to_string())
            }
            _ => GpuFrameError::Device(error.to_string()),
        })?;
        Ok(Self {
            device,
            renderer: None,
            target: None,
            images: FxHashMap::default(),
            textures: FxHashMap::default(),
        })
    }
    /// Adapter name and backend, for host diagnostics.
    /// 适配器名称与后端，供宿主诊断。
    pub fn adapter_info(&self) -> (String, String) {
        let info = self.device.adapter.get_info();
        (info.name, format!("{:?}", info.backend))
    }

    /// Register the host callback for a lost device. The host must recreate the device, Surface
    /// and renderer before drawing again; the engine view remains alive and dirty.
    /// 注册宿主设备丢失回调；宿主必须重建设备、Surface 与渲染器后才能继续绘制，引擎视图
    /// 保持存活且待重绘。
    pub fn set_device_lost_callback(&self, callback: impl Fn(GpuDeviceLost) + Send + 'static) {
        self.device
            .device
            .set_device_lost_callback(move |reason, message| {
                callback(GpuDeviceLost { reason, message });
            });
    }
    /// Paint `draw` into a fresh scene of `width`×`height`, render it and read it back.
    /// Callers admit the extent; dimensions above the device texture limit are rejected.
    /// 把 `draw` 画进 `width`×`height` 新场景、渲染并读回。尺寸准入由调用方完成；
    /// 超过设备纹理上限时拒绝。
    pub(crate) fn render(
        &mut self,
        width: u32,
        height: u32,
        draw: impl FnOnce(&mut VelloHybridScenePainter<'_>),
    ) -> Result<Vec<u8>, GpuFrameError> {
        let limit = self.device.device.limits().max_texture_dimension_2d;
        let Some((padded, buffer_bytes)) = gpu_readback_layout(width, height) else {
            return Err(GpuFrameError::Rejected);
        };
        if width > limit || height > limit {
            return Err(GpuFrameError::Rejected);
        }
        if self
            .target
            .as_ref()
            .is_none_or(|target| target.width != width || target.height != height)
        {
            self.target = Some(create_target(
                &self.device.device,
                width,
                height,
                buffer_bytes,
            ));
        }
        let target = self.target.take().expect("target created above");
        let painted = self.paint(
            width,
            height,
            TARGET_FORMAT,
            &target.view,
            &target.depth,
            Some((&target.texture, &target.readback, padded)),
            draw,
        );
        let readback = target.readback.clone();
        self.target = Some(target);
        painted?;
        read_back(&self.device.device, &readback, width, height, padded)
    }

    /// Render directly into a host-owned texture view. The host creates the Surface, configures
    /// its format/extent, calls this method, then presents through its own wgpu Surface.
    /// 直接绘制到宿主持有的纹理视图；宿主创建、配置 Surface，调用本方法后自行 present。
    pub(crate) fn render_to_host_view(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        draw: impl FnOnce(&mut VelloHybridScenePainter<'_>),
    ) -> Result<(), GpuFrameError> {
        let Some(_) = gpu_readback_layout(width, height) else {
            return Err(GpuFrameError::Rejected);
        };
        let limit = self.device.device.limits().max_texture_dimension_2d;
        if width > limit || height > limit {
            return Err(GpuFrameError::Rejected);
        }
        if !matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            return Err(GpuFrameError::Unsupported(format!(
                "host GPU target format {format:?}"
            )));
        }
        let depth = Renderer::create_depth_texture_view(
            &self.device.device,
            &RenderSize {
                width: width as u16,
                height: height as u16,
            },
        );
        self.paint(width, height, format, view, &depth, None, draw)
    }

    fn paint(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        copy_to: Option<(&wgpu::Texture, &wgpu::Buffer, u32)>,
        draw: impl FnOnce(&mut VelloHybridScenePainter<'_>),
    ) -> Result<(), GpuFrameError> {
        let key = RenderKey {
            format,
            width: width as u16,
            height: height as u16,
        };
        let (device, queue) = (&self.device.device, &self.device.queue);
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        if self
            .renderer
            .as_ref()
            .is_none_or(|(known, ..)| *known != key)
        {
            self.images.clear();
            self.textures.clear();
            let (renderer, resources) = Renderer::new_with(
                device,
                &RenderTargetConfig {
                    format,
                    width: key.width,
                    height: key.height,
                },
                RenderSettings::default(),
            );
            self.renderer = Some((key, renderer, resources));
        }
        let (_, renderer, resources) = self.renderer.as_mut().expect("renderer created above");
        let mut scene = Scene::new_with(key.width, key.height, RenderSettings::default().level);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("jasper native gpu frame"),
        });
        {
            let images = ImageManager::new(
                renderer,
                resources,
                device,
                queue,
                &mut encoder,
                &mut self.images,
            );
            let mut painter =
                VelloHybridScenePainter::new(&mut scene, images, &mut self.textures, &self.device);
            draw(&mut painter);
        }
        let mut bindings = TextureBindings::new();
        for (id, texture) in &self.textures {
            bindings.insert(vello_common::TextureId(id.into_ffi()), texture.clone());
        }
        renderer
            .render(
                &scene,
                resources,
                device,
                queue,
                &mut encoder,
                &RenderSize {
                    width: key.width,
                    height: key.height,
                },
                view,
                Some(depth),
                &bindings,
                TargetInit::Clear(ClearSettings::default()),
            )
            .map_err(|error| GpuFrameError::Render(format!("{error:?}")))?;
        if let Some((texture, buffer, padded)) = copy_to {
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: None,
                    },
                },
                texture.size(),
            );
        }
        queue.submit([encoder.finish()]);
        if let Some(error) = pollster::block_on(error_scope.pop()) {
            return Err(GpuFrameError::Render(error.to_string()));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RenderKey {
    format: wgpu::TextureFormat,
    width: u16,
    height: u16,
}

// BEGIN VERIFIED GPU READBACK LAYOUT KERNEL
/// Readback layout for a `width`×`height` RGBA8 frame: each row padded up to the 256-byte copy
/// alignment wgpu requires. Returns (padded row bytes, buffer bytes); None for an empty extent
/// or one beyond the 65535 frame limit shared with the CPU path.
/// `width`×`height` RGBA8 帧的读回布局：每行向上填充到 wgpu 要求的 256 字节拷贝对齐。返回
/// （填充后行字节数，缓冲字节数）；空尺寸或超出与 CPU 路径共用的 65535 上限时为 None。
pub fn gpu_readback_layout(width: u32, height: u32) -> Option<(u32, u64)> {
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return None;
    }
    let padded = (width * 4 + 255) / 256 * 256;
    Some((padded, padded as u64 * height as u64))
}
// END VERIFIED GPU READBACK LAYOUT KERNEL

fn create_target(device: &wgpu::Device, width: u32, height: u32, buffer_bytes: u64) -> Target {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("jasper native gpu target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = Renderer::create_depth_texture_view(
        device,
        &RenderSize {
            width: width as u16,
            height: height as u16,
        },
    );
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("jasper native gpu readback"),
        size: buffer_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    Target {
        width,
        height,
        texture,
        view,
        depth,
        readback,
    }
}

/// Map the readback buffer and strip the per-row copy alignment padding.
/// 映射读回缓冲并去掉每行拷贝对齐填充。
fn read_back(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    width: u32,
    height: u32,
    padded: u32,
) -> Result<Vec<u8>, GpuFrameError> {
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| GpuFrameError::Readback(error.to_string()))?;
    receiver
        .recv()
        .map_err(|error| GpuFrameError::Readback(error.to_string()))?
        .map_err(|error| GpuFrameError::Readback(error.to_string()))?;
    let (row, height, padded) = (width as usize * 4, height as usize, padded as usize);
    let mut pixels = Vec::with_capacity(row * height);
    let copied = {
        let data = slice
            .get_mapped_range()
            .map_err(|error| GpuFrameError::Readback(error.to_string()))?;
        (0..height).all(|line| {
            jasper_ui_engine::surface::surface_row(&data, row, height, padded, line)
                .map(|bytes| pixels.extend_from_slice(bytes))
                .is_some()
        })
    };
    buffer.unmap();
    if !copied {
        return Err(GpuFrameError::Readback(
            "mapped buffer shorter than its layout".into(),
        ));
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn device_destroy_notifies_the_host_once() {
        let gpu = match GpuRenderer::new_headless() {
            Ok(gpu) => gpu,
            Err(error) => {
                eprintln!("GPU unavailable; device-loss callback test skipped: {error:?}");
                return;
            }
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        gpu.set_device_lost_callback(move |loss| {
            let _ = sender.try_send(loss);
        });
        let GpuRenderer {
            device,
            renderer,
            target,
            images,
            textures,
        } = gpu;
        drop((renderer, target, images, textures));
        let DeviceHandle {
            instance,
            adapter,
            device,
            queue,
        } = device;
        device.destroy();
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        // Drop every remaining backend owner so the destroyed device can enqueue its callback.
        // 释放全部后端所有者，使已销毁设备能够提交回调。
        drop(queue);
        drop(device);
        drop(adapter);
        drop(instance);
        let loss = receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("device destruction must notify the host");
        assert_eq!(loss.reason, wgpu::DeviceLostReason::Destroyed);
    }
}
