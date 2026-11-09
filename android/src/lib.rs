//! Android NativeActivity reference host for the Jasper UI native engine.
//! Jasper UI 原生引擎的 Android NativeActivity 参考宿主。
//!
//! This crate owns the Android window, wgpu Surface/device/queue and Touch input. The engine
//! remains host-neutral and receives only owned scene values and normalized pointer events.
//! 本 crate 持有 Android 窗口、wgpu Surface/device/queue 与 Touch 输入；引擎保持宿主中立，
//! 只接收自有场景值与规范化指针事件。

#![cfg(target_os = "android")]

use std::sync::Arc;

use jasper_ui_engine::desktop::DesktopInput;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::input::{InputEvent, PointerPhase};
use jasper_ui_engine::native::host::{GpuRenderer, HostRenderer, SurfaceState};
use jasper_ui_engine::native::PointerAction;
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use wgpu_context::DeviceHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::android::activity::AndroidApp;
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::platform::android::WindowExtAndroid;
use winit::window::{Window, WindowId};

const OWNER: u64 = 0x4a53_5045_522d_414e;
const BUTTON: NodeId = NodeId(2);
const BUDGET: usize = 64 * 1024 * 1024;
const FONT: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

fn window_pixels(window: &Window) -> (u32, u32) {
    let content = window.content_rect();
    let width = content.right.saturating_sub(content.left) as u32;
    let height = content.bottom.saturating_sub(content.top) as u32;
    if width > 0 && height > 0 {
        (width, height)
    } else {
        let size = window.inner_size();
        (size.width, size.height)
    }
}

fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .width(Length::Percent(100.0))
                    .height(Length::Percent(100.0))
                    .padding(Edges::all(24.0))
                    .direction(FlexDirection::Row)
                    .justify_content(jasper_ui_engine::style::Justify::Center)
                    .align_items(jasper_ui_engine::style::Align::Center)
                    .background(Color::rgb(18, 20, 26)),
            )
            .child(
                Node::rect(BUTTON)
                    .with_semantics(
                        Semantics::default()
                            .role(Role::Button)
                            .label("Jasper Android")
                            .focusable(true),
                    )
                    .with_style(
                        Style::default()
                            .width(Length::Percent(70.0))
                            .height(Length::Px(180.0))
                            .background(Color::rgb(40, 130, 90)),
                    ),
            ),
    )
}

fn root_style(width: f32, height: f32) -> Style {
    Style::default()
        .width(Length::Px(width))
        .height(Length::Px(height))
        .padding(Edges::all(24.0))
        .direction(FlexDirection::Row)
        .justify_content(jasper_ui_engine::style::Justify::Center)
        .align_items(jasper_ui_engine::style::Align::Center)
        .background(Color::rgb(18, 20, 26))
}

struct App {
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    gpu: Option<GpuRenderer>,
    engine: HostRenderer,
    generation: u64,
    scene: Scene,
    input: DesktopInput,
    activations: u64,
    suspended: bool,
}

impl App {
    fn new() -> Option<Self> {
        let scene = scene();
        let engine = HostRenderer::new(OWNER, &scene, FONT, 640, 360)?;
        let generation = engine.surface_status(OWNER)?.generation;
        Some(Self {
            window: None,
            surface: None,
            config: None,
            device: None,
            queue: None,
            gpu: None,
            engine,
            generation,
            scene,
            input: DesktopInput::new(),
            activations: 0,
            suspended: false,
        })
    }

    fn configure_surface(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window = self.window.clone().ok_or("missing Android window")?;
        let (width, height) = window_pixels(&window);
        if width == 0 || height == 0 {
            return Ok(());
        }
        let device = self.device.as_ref().ok_or("missing wgpu device")?;
        let config = self.config.as_mut().ok_or("missing surface config")?;
        config.width = width;
        config.height = height;
        self.surface
            .as_ref()
            .ok_or("missing Android surface")?
            .configure(device, config);
        Ok(())
    }

    fn reconfigure_engine(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window = self.window.as_ref().ok_or("missing Android window")?;
        let (width, height) = window_pixels(&window);
        if width == 0 || height == 0 {
            return Ok(());
        }
        let scale = window.scale_factor() as f32;
        self.generation = self
            .engine
            .reconfigure(OWNER, self.generation, width, height, scale)?;
        self.resize_root(width, height, scale)?;
        Ok(())
    }

    fn resize_root(
        &mut self,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("Android DPI scale is invalid".into());
        }
        let old = self.scene.version();
        let patch = ScenePatch::UpdateStyle {
            id: NodeId(1),
            style: root_style(width as f32 / scale, height as f32 / scale),
        };
        self.scene
            .apply_batch_at(old, std::slice::from_ref(&patch))
            .map_err(|error| format!("Android root resize rejected: {error:?}"))?;
        if !self.engine.apply_with_focus_events(
            OWNER,
            old,
            self.scene.version(),
            self.generation,
            std::slice::from_ref(&patch),
            &mut |_| {},
        ) {
            return Err("native Android root resize rejected".into());
        }
        Ok(())
    }

    fn activate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.activations += 1;
        let background = if self.activations % 2 == 0 {
            Color::rgb(40, 130, 90)
        } else {
            Color::rgb(190, 92, 52)
        };
        let old = self.scene.version();
        let patch = ScenePatch::UpdateStyle {
            id: BUTTON,
            style: Style::default()
                .width(Length::Percent(70.0))
                .height(Length::Px(180.0))
                .background(background),
        };
        self.scene
            .apply_batch_at(old, std::slice::from_ref(&patch))
            .map_err(|error| format!("Android source update rejected: {error:?}"))?;
        if !self.engine.apply_with_focus_events(
            OWNER,
            old,
            self.scene.version(),
            self.generation,
            std::slice::from_ref(&patch),
            &mut |_| {},
        ) {
            return Err("native Android style update rejected".into());
        }
        log::info!("tap activation={}", self.activations);
        Ok(())
    }

    fn dispatch_inputs(
        &mut self,
        inputs: Vec<InputEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for input in inputs {
            if let InputEvent::Pointer(pointer) = input {
                let scale = self
                    .window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor() as f32);
                let action = match pointer.phase {
                    PointerPhase::Down => PointerAction::Down,
                    PointerPhase::Move | PointerPhase::Hover => PointerAction::Move,
                    PointerPhase::Up => PointerAction::Up,
                    PointerPhase::Cancel => PointerAction::Cancel,
                };
                let mut activated = false;
                if !self.engine.pointer(
                    OWNER,
                    self.scene.version(),
                    self.generation,
                    action,
                    pointer.position.x / scale,
                    pointer.position.y / scale,
                    matches!(pointer.phase, PointerPhase::Down | PointerPhase::Move),
                    &mut |event| {
                        activated |=
                            event.kind == jasper_ui_engine::native::SceneEventKind::Activate;
                        jasper_ui_engine::native::EventReply::default()
                    },
                    &mut |_| {},
                ) {
                    return Err("native Android pointer rejected".into());
                }
                if activated {
                    self.activate()?;
                }
            }
        }
        Ok(())
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window = self.window.clone().ok_or("missing Android window")?;
        let (width, height) = window_pixels(&window);
        if width == 0 || height == 0 {
            return Ok(());
        }
        let format = self.config.as_ref().ok_or("missing surface config")?.format;
        let frame = match self
            .surface
            .as_ref()
            .ok_or("missing Android surface")?
            .get_current_texture()
        {
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
                return Err("Android surface validation failed".into())
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.engine
            .render_gpu_to_host_view(
                OWNER,
                self.scene.version(),
                self.generation,
                self.gpu.as_mut().ok_or("missing GPU renderer")?,
                format,
                &view,
                BUDGET,
            )
            .map_err(|error| format!("Android GPU frame rejected: {error:?}"))?;
        self.queue
            .as_ref()
            .ok_or("missing GPU queue")?
            .present(frame);
        if !self.engine.gpu_presented(OWNER, self.generation) {
            return Err("Android GPU present acknowledgement rejected".into());
        }
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        self.suspended = false;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let window = Arc::new(
                event_loop
                    .create_window(Window::default_attributes().with_title("Jasper UI Android"))?,
            );
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let surface = instance.create_surface(window.clone())?;
            let host = pollster::block_on(DeviceHandle::new_from_compatible_surface(
                instance,
                Some(&surface),
                None,
                None,
            ))
            .map_err(|error| format!("wgpu Android device: {error}"))?;
            let caps = surface.get_capabilities(&host.adapter);
            let format = [
                wgpu::TextureFormat::Bgra8UnormSrgb,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureFormat::Rgba8Unorm,
            ]
            .into_iter()
            .find(|format| caps.formats.contains(format))
            .ok_or("no supported Android surface format")?;
            let (width, height) = window_pixels(&window);
            let mut config = surface
                .get_default_config(&host.adapter, width.max(1), height.max(1))
                .ok_or("Android surface configuration")?;
            config.format = format;
            config.present_mode = wgpu::PresentMode::AutoVsync;
            surface.configure(&host.device, &config);
            let gpu = GpuRenderer::from_wgpu(
                host.instance,
                host.adapter,
                host.device.clone(),
                host.queue.clone(),
            );
            let state = self
                .engine
                .surface_status(OWNER)
                .ok_or("missing engine surface")?;
            log::info!(
                "Android resume from state {:?} generation {}",
                state.state,
                self.generation
            );
            self.generation = if state.state == SurfaceState::Suspended {
                self.engine.resume(
                    OWNER,
                    self.generation,
                    width.max(1),
                    height.max(1),
                    window.scale_factor() as f32,
                )?
            } else {
                self.engine.reconfigure(
                    OWNER,
                    self.generation,
                    width.max(1),
                    height.max(1),
                    window.scale_factor() as f32,
                )?
            };
            self.resize_root(width, height, window.scale_factor() as f32)?;
            self.device = Some(host.device);
            self.queue = Some(host.queue);
            self.surface = Some(surface);
            self.config = Some(config);
            self.gpu = Some(gpu);
            self.window = Some(window.clone());
            log::info!("Android content size {width}x{height}");
            window.request_redraw();
            Ok(())
        })();
        if let Err(error) = result {
            log::error!("Android resume failed: {error}");
            event_loop.exit();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.suspended = true;
        log::info!("Android suspend generation {}", self.generation);
        match self
            .engine
            .suspend(OWNER, self.scene.version(), self.generation, &mut |_| {})
        {
            Ok(generation) => {
                self.generation = generation;
                log::info!("Android suspended at generation {generation}");
            }
            Err(error) => log::error!("Android suspend failed: {error}"),
        }
        self.surface = None;
        self.config = None;
        self.gpu = None;
        self.queue = None;
        self.device = None;
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
                let _ =
                    self.engine
                        .retire(OWNER, self.scene.version(), self.generation, &mut |_| {});
                event_loop.exit();
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Err(error) = self.configure_surface() {
                    log::error!("Android surface resize failed: {error}");
                    event_loop.exit();
                } else if let Err(error) = self.reconfigure_engine() {
                    log::error!("Android viewport resize failed: {error}");
                    event_loop.exit();
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.render() {
                    log::error!("Android frame failed: {error}");
                    event_loop.exit();
                }
            }
            other => {
                let inputs = self.input.translate(&other);
                if inputs.is_empty() {
                    return;
                }
                if let Err(error) = self.dispatch_inputs(inputs) {
                    log::error!("Android input failed: {error}");
                    event_loop.exit();
                    return;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.suspended {
            let _ = self
                .engine
                .retire(OWNER, self.scene.version(), self.generation, &mut |_| {});
        }
    }
}

#[no_mangle]
pub extern "C" fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("JasperUI"),
    );
    log::info!("android_main entered");
    let event_loop = match EventLoop::<()>::with_user_event()
        .with_android_app(app)
        .build()
    {
        Ok(event_loop) => {
            log::info!("event loop built");
            event_loop
        }
        Err(error) => {
            log::error!("Android event loop creation failed: {error}");
            return;
        }
    };
    let Some(mut app) = App::new() else {
        log::error!("Android native scene import failed");
        return;
    };
    log::info!("native scene ready");
    if let Err(error) = event_loop.run_app(&mut app) {
        log::error!("Android event loop failed: {error}");
    }
}
