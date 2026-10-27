//! Host-owned winit/softbuffer presentation of the root native CPU candidate.
//! 宿主持有窗口与 Surface，根原生候选负责绘制；完整验收尚未完成。
//! Run: cargo run --features desktop,native-blitz-cpu-pending --example window -- FONT [--once].

use std::num::NonZeroU32;
use std::sync::Arc;

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::desktop::{rgba_to_u32, DesktopInput};
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    engine: Engine,
    backend: HeadlessBackend,
    scene: Scene,
    input: DesktopInput,
    once: bool,
    presentation_error: Option<String>,
}

impl App {
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
        let engine = Engine::new(10, &scene, font, 640, 360)
            .ok_or("native scene import rejected")?;
        Ok(Self {
            window: None,
            surface: None,
            engine,
            backend: HeadlessBackend::new(1, 1),
            scene,
            input: DesktopInput::new(),
            once,
            presentation_error: None,
        })
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        surface.resize(width, height)?;
        // Window size is physical; native layout and events use logical coordinates.
        // 窗口尺寸为物理像素，原生布局与事件使用逻辑坐标。
        if !self.engine.set_viewport(10, size.width, size.height, window.scale_factor() as f32) {
            return Err("native viewport rejected".into());
        }
        self.engine.render_to_backend(10, self.scene.version(), &mut self.backend, 64 * 1024 * 1024)
            .map_err(|error| format!("native CPU frame rejected: {error:?}"))?;
        let pixels = self.backend.canvas().pixels();
        let mut buffer = surface.buffer_mut()?;
        rgba_to_u32(pixels, &mut buffer).map_err(|error| format!("pixel conversion failed: {error:?}"))?;
        buffer.present()?;
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let attributes = Window::default_attributes().with_title("Jasper UI Engine");
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        let context = Context::new(window.clone()).expect("softbuffer context");
        let surface = Surface::new(&context, window.clone()).expect("softbuffer surface");
        window.request_redraw();
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // Native state survives host suspension; surface ownership stays here.
        // 宿主暂停保留原生场景状态，仅在本层释放窗口与 Surface。
        self.engine.deactivate(10, self.scene.version(), &mut |_| {});
        self.surface = None;
        self.window = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) { return; }
        match event {
            WindowEvent::CloseRequested => {
                self.engine.close(10);
                event_loop.exit();
            },
            WindowEvent::Focused(false) => {
                self.engine.deactivate(10, self.scene.version(), &mut |_| {});
            },
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.render() {
                    self.presentation_error = Some(error.to_string());
                    eprintln!("presentation failed: {error}");
                    event_loop.exit();
                    return;
                }
                if self.once {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            other => {
                let events = self.input.translate(&other);
                if !events.is_empty() {
                    eprintln!("input: {events:?}");
                }
            }
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
    if arguments.next().is_some() { return Err("expected FONT [--once]".into()); }
    let font = std::fs::read(font_path)?;
    let event_loop = EventLoop::new()?;
    let mut app = App::new(once, &font)?;
    let result = event_loop.run_app(&mut app);
    app.engine.close(10);
    result?;
    if let Some(error) = app.presentation_error { return Err(error.into()); }
    Ok(())
}
