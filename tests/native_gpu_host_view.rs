//! Host-owned wgpu device and texture acceptance for direct GPU presentation.
//! 宿主持有 wgpu 设备与纹理的直接 GPU 呈现验收。
#![cfg(feature = "gpu-pending")]
use jasper_ui_engine::native::host::{GpuFrameError, GpuRenderer, HostRenderer};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, Length, Style};
use wgpu_context::DeviceHandle;

const W: u32 = 64;
const H: u32 = 64;
const OWNER: u64 = 51;
const BUDGET: usize = (W * H * 4) as usize;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn font() -> Vec<u8> {
    let path = std::env::var("JASPER_UI_TEST_FONT")
        .expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .background(Color::rgb(10, 20, 30)),
            )
            .child(
                Node::rect(NodeId(2)).with_style(
                    Style::default()
                        .size(Length::Px(20.0), Length::Px(20.0))
                        .background(Color::RED),
                ),
            ),
    )
}

#[test]
fn host_device_and_texture_are_used_without_engine_device_acquisition() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let host = pollster::block_on(DeviceHandle::new_from_compatible_surface(
        instance, None, None, None,
    ))
    .expect("host wgpu device");
    let host_info = host.adapter.get_info();
    let device = host.device.clone();
    let queue = host.queue.clone();
    let mut gpu = GpuRenderer::from_wgpu(host.instance, host.adapter, host.device, host.queue);
    assert_eq!(gpu.adapter_info().0, host_info.name);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("host presentation target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let source = scene();
    let mut engine = HostRenderer::new(OWNER, &source, &font(), W, H).expect("native import");
    let generation = engine.surface_status(OWNER).unwrap().generation;
    assert_eq!(engine.needs_frame(OWNER, generation), Some(true));
    assert_eq!(
        engine.render_gpu_to_host_view(
            OWNER + 1,
            source.version(),
            generation,
            &mut gpu,
            FORMAT,
            &view,
            BUDGET
        ),
        Err(GpuFrameError::Rejected)
    );
    assert_eq!(
        engine.needs_frame(OWNER, generation),
        Some(true),
        "foreign rejection must not change scheduling"
    );
    engine
        .render_gpu_to_host_view(
            OWNER,
            source.version(),
            generation,
            &mut gpu,
            FORMAT,
            &view,
            BUDGET,
        )
        .expect("direct host view render");
    assert_eq!(
        engine.needs_frame(OWNER, generation),
        Some(true),
        "render is not present"
    );
    assert!(engine.gpu_presented(OWNER, generation));
    assert_eq!(engine.needs_frame(OWNER, generation), Some(false));

    // Host reads its own texture back. The engine never owns or presents this target.
    let row = W * 4;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("host test readback"),
        size: (row * H) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("host test copy"),
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("host poll");
    receiver
        .recv()
        .expect("map callback")
        .expect("host readback map");
    let pixels = slice.get_mapped_range().expect("mapped host texture");
    assert_eq!(
        &pixels[0..4],
        [Color::RED.r, Color::RED.g, Color::RED.b, Color::RED.a]
    );
    assert_eq!(
        &pixels[(32 * row + 32 * 4) as usize..(32 * row + 32 * 4) as usize + 4],
        [10, 20, 30, 255]
    );
    drop(pixels);
    readback.unmap();
}

#[test]
fn unsupported_host_format_is_explicit_and_fails_closed() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let host = pollster::block_on(DeviceHandle::new_from_compatible_surface(
        instance, None, None, None,
    ))
    .expect("host wgpu device");
    let device = host.device.clone();
    let mut gpu = GpuRenderer::from_wgpu(host.instance, host.adapter, host.device, host.queue);
    let format = wgpu::TextureFormat::Rgba16Float;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("unsupported target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let source = scene();
    let mut engine = HostRenderer::new(OWNER, &source, &font(), W, H).expect("native import");
    let generation = engine.surface_status(OWNER).unwrap().generation;
    assert!(matches!(
        engine.render_gpu_to_host_view(
            OWNER,
            source.version(),
            generation,
            &mut gpu,
            format,
            &view,
            BUDGET
        ),
        Err(GpuFrameError::Unsupported(_))
    ));
    assert_eq!(engine.needs_frame(OWNER, generation), Some(true));
}
