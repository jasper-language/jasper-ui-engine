//! Host-owned winit/wgpu Surface presented by the root Engine's native GPU path.
//! Run: cargo run --features desktop,gpu-pending --example gpu_window -- FONT [--once].
use std::sync::Arc;

use jasper_ui_engine::native::host::{GpuRenderer, HostRenderer, SurfaceState};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use wgpu_context::DeviceHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

const OWNER: u64 = 10;
const BUDGET: usize = 64 * 1024 * 1024;

#[derive(Debug)]
enum AppEvent {
    DeviceLost {
        generation: u64,
        reason: wgpu::DeviceLostReason,
        message: String,
    },
    LifecycleDrill {
        generation: u64,
    },
}

fn supported_format(caps: &wgpu::SurfaceCapabilities) -> Option<wgpu::TextureFormat> {
    [
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ]
    .into_iter()
    .find(|format| caps.formats.contains(format))
}

struct GpuApp {
    event_proxy: EventLoopProxy<AppEvent>,
    window: Option<Arc<Window>>,
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    gpu: Option<GpuRenderer>,
    engine: HostRenderer,
    generation: u64,
    scene: Scene,
    once: bool,
    exercise_device_loss: bool,
    exercise_suspend_resume: bool,
    frames_presented: usize,
    recovery_exercised: bool,
    suspend_resume_exercised: bool,
    recovering: bool,
    device_loss_pending: bool,
    suspend_resume_pending: bool,
    closing: bool,
    device_generation: u64,
    presentation_error: Option<String>,
}

impl GpuApp {
    fn new(
        once: bool,
        exercise_device_loss: bool,
        exercise_suspend_resume: bool,
        font: &[u8],
        event_proxy: EventLoopProxy<AppEvent>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let scene = Scene::new(
            Node::container(NodeId(1))
                .with_style(
                    Style::default()
                        .background(Color::new(24, 26, 32, 255))
                        .padding(jasper_ui_engine::geometry::Edges::all(32.0))
                        .gap(24.0)
                        .direction(FlexDirection::Row),
                )
                .child(
                    Node::rect(NodeId(2)).with_style(
                        Style::default()
                            .size(Length::Px(200.0), Length::Px(120.0))
                            .background(Color::RED),
                    ),
                )
                .child(
                    Node::rect(NodeId(3)).with_style(
                        Style::default()
                            .size(Length::Px(200.0), Length::Px(120.0))
                            .background(Color::BLUE),
                    ),
                ),
        );
        let engine = HostRenderer::new(OWNER, &scene, font, 640, 360)
            .ok_or("native scene import rejected")?;
        let generation = engine
            .surface_status(OWNER)
            .ok_or("native surface missing")?
            .generation;
        Ok(Self {
            event_proxy,
            window: None,
            instance: None,
            adapter: None,
            surface: None,
            config: None,
            device: None,
            queue: None,
            gpu: None,
            engine,
            generation,
            scene,
            once,
            exercise_device_loss,
            exercise_suspend_resume,
            frames_presented: 0,
            recovery_exercised: false,
            suspend_resume_exercised: false,
            recovering: false,
            device_loss_pending: false,
            suspend_resume_pending: false,
            closing: false,
            device_generation: 0,
            presentation_error: None,
        })
    }

    fn preferred_present_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::PresentMode {
        let requested = match std::env::var("JASPER_UI_PRESENT_MODE")
            .unwrap_or_else(|_| "fifo".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "mailbox" => wgpu::PresentMode::Mailbox,
            "immediate" => wgpu::PresentMode::Immediate,
            "auto-vsync" | "autovsync" => wgpu::PresentMode::AutoVsync,
            "auto-no-vsync" | "autonovsync" => wgpu::PresentMode::AutoNoVsync,
            _ => wgpu::PresentMode::Fifo,
        };
        if caps.present_modes.contains(&requested) {
            requested
        } else {
            wgpu::PresentMode::Fifo
        }
    }

    fn install_device_loss_callback(&self, gpu: &GpuRenderer) {
        let proxy = self.event_proxy.clone();
        let generation = self.device_generation;
        gpu.set_device_lost_callback(move |loss| {
            let _ = proxy.send_event(AppEvent::DeviceLost {
                generation,
                reason: loss.reason,
                message: loss.message,
            });
        });
    }

    fn drop_gpu_resources(&mut self) {
        self.surface = None;
        self.config = None;
        self.gpu = None;
        self.queue = None;
        self.device = None;
        self.adapter = None;
        self.instance = None;
    }

    fn acquire_gpu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window = self.window.clone().ok_or("missing host window")?;
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone())?;
        let host = pollster::block_on(DeviceHandle::new_from_compatible_surface(
            instance,
            Some(&surface),
            None,
            None,
        ))
        .map_err(|error| format!("host wgpu device: {error}"))?;
        let caps = surface.get_capabilities(&host.adapter);
        let format = supported_format(&caps).ok_or("no supported host surface format")?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&host.adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface configuration")?;
        config.format = format;
        config.present_mode = Self::preferred_present_mode(&caps);
        surface.configure(&host.device, &config);
        let gpu = GpuRenderer::from_wgpu(
            host.instance.clone(),
            host.adapter.clone(),
            host.device.clone(),
            host.queue.clone(),
        );
        self.install_device_loss_callback(&gpu);
        self.device_generation = self.device_generation.saturating_add(1);
        println!(
            "GPU device generation {}: adapter={:?} format={:?} present={:?}",
            self.device_generation,
            gpu.adapter_info(),
            config.format,
            config.present_mode
        );
        self.instance = Some(host.instance);
        self.adapter = Some(host.adapter);
        self.device = Some(host.device);
        self.queue = Some(host.queue);
        self.surface = Some(surface);
        self.config = Some(config);
        self.gpu = Some(gpu);

        let scale = window.scale_factor() as f32;
        let state = self
            .engine
            .surface_status(OWNER)
            .ok_or("native surface missing")?;
        self.generation = if state.state == SurfaceState::Suspended {
            self.engine.resume(
                OWNER,
                self.generation,
                size.width.max(1),
                size.height.max(1),
                scale,
            )?
        } else {
            self.engine.reconfigure(
                OWNER,
                self.generation,
                size.width.max(1),
                size.height.max(1),
                scale,
            )?
        };
        Ok(())
    }

    fn recover_surface(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window = self.window.clone().ok_or("missing host window")?;
        let instance = self.instance.clone().ok_or("missing wgpu instance")?;
        let adapter = self.adapter.as_ref().ok_or("missing wgpu adapter")?;
        let device = self.device.as_ref().ok_or("missing GPU device")?;
        let surface = instance.create_surface(window.clone())?;
        let caps = surface.get_capabilities(adapter);
        let format = supported_format(&caps).ok_or("no supported host surface format")?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface configuration")?;
        config.format = format;
        config.present_mode = Self::preferred_present_mode(&caps);
        surface.configure(device, &config);
        self.surface = Some(surface);
        self.config = Some(config);
        self.reconfigure_engine()
    }

    fn recreate_host(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = self
            .engine
            .surface_status(OWNER)
            .ok_or("native surface missing")?;
        if state.state == SurfaceState::Active {
            self.generation =
                self.engine
                    .suspend(OWNER, self.scene.version(), self.generation, &mut |_| {})?;
        }
        self.window = None;
        self.drop_gpu_resources();
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Jasper UI Engine GPU"))?,
        );
        self.window = Some(window.clone());
        self.acquire_gpu()?;
        window.request_redraw();
        Ok(())
    }

    fn configure_surface(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let device = self.device.as_ref().ok_or("missing GPU device")?;
        let config = self
            .config
            .as_mut()
            .ok_or("missing GPU surface configuration")?;
        config.width = size.width;
        config.height = size.height;
        self.surface
            .as_ref()
            .ok_or("missing GPU surface")?
            .configure(device, config);
        Ok(())
    }

    fn reconfigure_engine(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        self.generation = self
            .engine
            .reconfigure(
                OWNER,
                self.generation,
                size.width,
                size.height,
                window.scale_factor() as f32,
            )
            .map_err(|error| format!("native surface reconfigure rejected: {error:?}"))?;
        Ok(())
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.device_loss_pending || self.suspend_resume_pending {
            return Ok(());
        }
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let surface = self.surface.as_ref().ok_or("missing GPU surface")?;
        let config = self
            .config
            .as_ref()
            .ok_or("missing GPU surface configuration")?;
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure_surface()?;
                self.reconfigure_engine()?;
                window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.recover_surface()?;
                window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(())
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                if self.device_loss_pending {
                    return Ok(());
                }
                return Err("surface validation failure".into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let gpu = self.gpu.as_mut().ok_or("missing native GPU renderer")?;
        self.engine
            .render_gpu_to_host_view(
                OWNER,
                self.scene.version(),
                self.generation,
                gpu,
                config.format,
                &view,
                BUDGET,
            )
            .map_err(|error| format!("native GPU frame rejected: {error:?}"))?;
        self.queue
            .as_ref()
            .ok_or("missing GPU queue")?
            .present(frame);
        if !self.engine.gpu_presented(OWNER, self.generation) {
            return Err("GPU present acknowledgement rejected".into());
        }
        self.frames_presented += 1;
        eprintln!(
            "GPU frame presented: frame={} device_generation={}",
            self.frames_presented, self.device_generation
        );
        if self.exercise_device_loss && !self.recovery_exercised {
            self.recovery_exercised = true;
            self.device_loss_pending = true;
            eprintln!(
                "GPU device-loss recovery drill: rebuilding generation {} after frame {}",
                self.device_generation, self.frames_presented
            );
            // Route the drill through the same event path as a real callback without forcing
            // a backend-specific driver failure.
            // 演练经真实回调相同的事件路径触发生命周期重建，但不伪造后端驱动故障。
            let _ = self.event_proxy.send_event(AppEvent::DeviceLost {
                generation: self.device_generation,
                reason: wgpu::DeviceLostReason::Unknown,
                message: "host recovery drill".into(),
            });
        } else if self.exercise_suspend_resume && !self.suspend_resume_exercised {
            self.suspend_resume_exercised = true;
            self.suspend_resume_pending = true;
            eprintln!(
                "GPU suspend/resume drill: rebuilding generation {} after frame {}",
                self.device_generation, self.frames_presented
            );
            let _ = self.event_proxy.send_event(AppEvent::LifecycleDrill {
                generation: self.device_generation,
            });
        }
        Ok(())
    }
}

impl ApplicationHandler<AppEvent> for GpuApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.closing {
            return;
        }
        let window = match event_loop
            .create_window(Window::default_attributes().with_title("Jasper UI Engine GPU"))
        {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.presentation_error = Some(format!("window creation failed: {error}"));
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window.clone());
        if let Err(error) = self.acquire_gpu() {
            self.presentation_error = Some(error.to_string());
            event_loop.exit();
            return;
        }
        window.request_redraw();
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        match self
            .engine
            .suspend(OWNER, self.scene.version(), self.generation, &mut |_| {})
        {
            Ok(generation) => self.generation = generation,
            Err(error) => {
                self.presentation_error = Some(format!("native suspend rejected: {error:?}"));
                event_loop.exit();
                return;
            }
        }
        self.surface = None;
        self.config = None;
        self.device = None;
        self.queue = None;
        self.gpu = None;
        self.adapter = None;
        self.instance = None;
        self.window = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.closing = true;
                let _ =
                    self.engine
                        .retire(OWNER, self.scene.version(), self.generation, &mut |_| {});
                event_loop.exit();
            }
            WindowEvent::Focused(false) => {}
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Err(error) = self.configure_surface() {
                    self.presentation_error = Some(error.to_string());
                    event_loop.exit();
                    return;
                }
                if let Err(error) = self.reconfigure_engine() {
                    self.presentation_error = Some(error.to_string());
                    event_loop.exit();
                    return;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.render() {
                    eprintln!("GPU presentation failed: {error}");
                    self.presentation_error = Some(error.to_string());
                    event_loop.exit();
                    return;
                }
                let required_frames = if self.exercise_device_loss || self.exercise_suspend_resume {
                    2
                } else if self.once {
                    1
                } else {
                    0
                };
                if required_frames > 0 && self.frames_presented >= required_frames {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::DeviceLost {
                generation,
                reason,
                message,
            } => {
                if self.closing || self.recovering || generation != self.device_generation {
                    return;
                }
                eprintln!("GPU device lost: reason={reason:?} message={message:?}");
                self.recovering = true;
                let recovered = self.recreate_host(event_loop);
                self.recovering = false;
                self.device_loss_pending = false;
                if let Err(error) = recovered {
                    self.presentation_error = Some(format!("GPU device recovery failed: {error}"));
                    event_loop.exit();
                    return;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            AppEvent::LifecycleDrill { generation } => {
                if self.closing || self.recovering || generation != self.device_generation {
                    return;
                }
                self.recovering = true;
                let recovered = self.recreate_host(event_loop);
                self.recovering = false;
                self.suspend_resume_pending = false;
                if let Err(error) = recovered {
                    self.presentation_error =
                        Some(format!("GPU suspend/resume drill failed: {error}"));
                    event_loop.exit();
                    return;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let font_path = arguments.next().ok_or("supply an injected font file")?;
    let mut once = false;
    let mut exercise_device_loss = false;
    let mut exercise_suspend_resume = false;
    for argument in arguments {
        match argument.as_str() {
            "--once" => once = true,
            "--exercise-device-loss" => exercise_device_loss = true,
            "--exercise-suspend-resume" => exercise_suspend_resume = true,
            _ => {
                return Err(
                    "expected FONT [--once] [--exercise-device-loss|--exercise-suspend-resume]"
                        .into(),
                )
            }
        }
    }
    let font = std::fs::read(font_path)?;
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let event_proxy = event_loop.create_proxy();
    let mut app = GpuApp::new(
        once,
        exercise_device_loss,
        exercise_suspend_resume,
        &font,
        event_proxy,
    )?;
    let result = event_loop.run_app(&mut app);
    app.engine.close(OWNER, app.scene.version(), &mut |_| {});
    if let Err(error) = &result {
        eprintln!("event loop failed: {error:?}");
    }
    result?;
    if let Some(error) = app.presentation_error {
        return Err(error.into());
    }
    println!(
        "gpu_window=pass frames={} device_generation={} recovery_exercised={} suspend_resume_exercised={}",
        app.frames_presented,
        app.device_generation,
        app.recovery_exercised,
        app.suspend_resume_exercised
    );
    Ok(())
}
