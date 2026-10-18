//! Real winit window with software presentation of one engine frame.
//! Run with `cargo run --features desktop --example window -- --once`.

use std::num::NonZeroU32;
use std::sync::Arc;

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::desktop::{rgba_to_u32, DesktopInput};
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Size;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use jasper_ui_engine::text::SimpleTextEngine;
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    engine: Engine<HeadlessBackend>,
    scene: Scene,
    input: DesktopInput,
    once: bool,
}

impl App {
    fn new(once: bool) -> Self {
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
        Self {
            window: None,
            surface: None,
            engine: Engine::new(
                Size::new(640.0, 360.0),
                HeadlessBackend::new(640, 360),
                Box::new(SimpleTextEngine),
            )
            .with_clear_color(Color::new(24, 26, 32, 255)),
            scene,
            input: DesktopInput::new(),
            once,
        }
    }

    fn render(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        surface.resize(width, height).expect("resize softbuffer");
        self.engine
            .resize(Size::new(size.width as f32, size.height as f32));
        self.engine.render(&self.scene).expect("render");
        let pixels = self.engine.backend().canvas().pixels();
        let mut buffer = surface.buffer_mut().expect("softbuffer buffer");
        rgba_to_u32(pixels, &mut buffer).expect("pixel conversion");
        buffer.present().expect("present");
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = Window::default_attributes().with_title("Jasper UI Engine");
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        let context = Context::new(window.clone()).expect("softbuffer context");
        let surface = Surface::new(&context, window.clone()).expect("softbuffer surface");
        window.request_redraw();
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                self.render();
                if self.once {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) => {
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

fn main() {
    let once = std::env::args().any(|argument| argument == "--once");
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::new(once);
    event_loop.run_app(&mut app).expect("run app");
}
