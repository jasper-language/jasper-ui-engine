//! Host-owned winit/wgpu Surface presented by the root Engine's native GPU path.
//! Run: cargo run --features desktop,gpu-pending --example gpu_window -- FONT [--once].
use std::sync::Arc;

use jasper_ui_engine::native::host::{GpuRenderer, HostRenderer, SurfaceState};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use wgpu_context::DeviceHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const OWNER: u64 = 10;
const BUDGET: usize = 64 * 1024 * 1024;

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
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    gpu: Option<GpuRenderer>,
    engine: HostRenderer,
    generation: u64,
    scene: Scene,
    once: bool,
    presentation_error: Option<String>,
}

impl GpuApp {
    fn new(once: bool, font: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
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
        let engine =
            HostRenderer::new(OWNER, &scene, font, 640, 360).ok_or("native scene import rejected")?;
        let generation = engine
            .surface_status(OWNER)
            .ok_or("native surface missing")?
            .generation;
        Ok(Self {
            window: None,
            surface: None,
            config: None,
            device: None,
            queue: None,
            gpu: None,
            engine,
            generation,
            scene,
            once,
            presentation_error: None,
        })
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
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure_surface()?;
                self.reconfigure_engine()?;
                window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(())
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface validation failure".into())
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
        Ok(())
    }
}

impl ApplicationHandler for GpuApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Jasper UI Engine GPU"))
                .expect("create window"),
        );
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance
            .create_surface(window.clone())
            .expect("wgpu surface");
        let host = pollster::block_on(DeviceHandle::new_from_compatible_surface(
            instance,
            Some(&surface),
            None,
            None,
        ))
        .expect("host wgpu device");
        let caps = surface.get_capabilities(&host.adapter);
        let Some(format) = supported_format(&caps) else {
            panic!("no supported host surface format");
        };
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&host.adapter, size.width.max(1), size.height.max(1))
            .expect("surface configuration");
        config.format = format;
        surface.configure(&host.device, &config);
        let gpu = GpuRenderer::from_wgpu(
            host.instance,
            host.adapter,
            host.device.clone(),
            host.queue.clone(),
        );
        println!("GPU adapter: {:?}", gpu.adapter_info());
        self.gpu = Some(gpu);
        self.device = Some(host.device);
        self.queue = Some(host.queue);
        self.surface = Some(surface);
        self.config = Some(config);
        self.window = Some(window.clone());
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let Some(state) = self.engine.surface_status(OWNER) else {
            self.presentation_error = Some("native surface missing".into());
            event_loop.exit();
            return;
        };
        let generation = if state.state == SurfaceState::Suspended {
            self.engine.resume(OWNER, self.generation, size.width.max(1), size.height.max(1), scale)
        } else {
            self.engine.reconfigure(OWNER, self.generation, size.width.max(1), size.height.max(1), scale)
        };
        match generation {
            Ok(generation) => self.generation = generation,
            Err(error) => {
                self.presentation_error = Some(format!("native surface activation rejected: {error:?}"));
                event_loop.exit();
                return;
            }
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
                let _ = self.engine.retire(
                    OWNER,
                    self.scene.version(),
                    self.generation,
                    &mut |_| {},
                );
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
                if self.once {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let font_path = arguments.next().ok_or("supply an injected font file")?;
    let once = match arguments.next().as_deref() {
        None => false,
        Some("--once") => true,
        _ => return Err("expected FONT [--once]".into()),
    };
    if arguments.next().is_some() {
        return Err("expected FONT [--once]".into());
    }
    let font = std::fs::read(font_path)?;
    let event_loop = EventLoop::new()?;
    let mut app = GpuApp::new(once, &font)?;
    let result = event_loop.run_app(&mut app);
    app.engine.close(OWNER, app.scene.version(), &mut |_| {});
    result?;
    if let Some(error) = app.presentation_error {
        return Err(error.into());
    }
    Ok(())
}
