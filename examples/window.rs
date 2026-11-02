//! Host-owned winit/softbuffer window for the public full UI lifecycle.
//! 使用公共完整 UI 生命周期的宿主 winit/softbuffer 窗口。
//!
//! The host acquires the window, Surface, clock and platform events. Jasper declarations,
//! updates, layout, presentation, interaction, external text/IME display and retirement all
//! use the root `engine::Engine` / `native::host` facades. The external text snapshot is a
//! deliberately injected service fixture; this example does not implement an editor or IME.
//!
//! Run:
//! `cargo run --locked --features desktop,native-engine-cpu --example window -- FONT [--once|--once-after-activation]`

use std::num::NonZeroU32;
use std::sync::Arc;

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::desktop::{rgba_to_u32, DesktopInput};
use jasper_ui_engine::input::{InputEvent, KeyPhase, PointerPhase};
use jasper_ui_engine::native::host::{
    HostRenderer, SurfaceState, TextDisplayView, UiCompositionToken, UiInputBinding,
    UiInputContext, UiInputTicket,
};
use jasper_ui_engine::native::{EventReply, HostKeyInput, KeyLocation, PointerAction};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{Color, FlexDirection, FontStyle, Length, Style};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const SCENE_OWNER: u64 = 10;
const TEXT_OWNER: u64 = 20;
const BUDGET: usize = 64 * 1024 * 1024;
const BUTTON: NodeId = NodeId(2);
const TEXT_TARGET: NodeId = NodeId(9);

struct App {
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    engine: HostRenderer,
    generation: u64,
    viewport: Option<(u32, u32, f32)>,
    backend: HeadlessBackend,
    scene: Scene,
    input: DesktopInput,
    text: TextDisplayView,
    text_binding: UiInputBinding,
    text_context: UiInputContext,
    text_ticket: Option<UiInputTicket>,
    text_preedit: Option<UiCompositionToken>,
    text_snapshot: String,
    external_dirty: bool,
    scroll_y: f64,
    activated: usize,
    once: bool,
    exit_after_activation: bool,
    presentation_error: Option<String>,
    closed: bool,
}

fn scene() -> Scene {
    let button = Node::text(BUTTON, "Activate")
        .with_semantics(
            Semantics::default()
                .role(Role::Button)
                .label("Activate")
                .focusable(true),
        )
        .with_style(
            Style::default()
                .size(Length::Px(200.0), Length::Px(52.0))
                .background(Color::rgb(40, 130, 90)),
        );
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .background(Color::new(24, 26, 32, 255))
                    .padding(jasper_ui_engine::geometry::Edges::all(32.0))
                    .gap(24.0)
                    .direction(FlexDirection::Column),
            )
            .child(button),
    )
}

fn command_code(key: &str) -> Option<&'static str> {
    match key {
        "Enter" => Some("Enter"),
        "Tab" => Some("Tab"),
        "Escape" => Some("Escape"),
        "Space" => Some("Space"),
        "Backspace" => Some("Backspace"),
        "Delete" => Some("Delete"),
        "Insert" => Some("Insert"),
        "Home" => Some("Home"),
        "End" => Some("End"),
        "PageUp" => Some("PageUp"),
        "PageDown" => Some("PageDown"),
        "ArrowUp" => Some("ArrowUp"),
        "ArrowDown" => Some("ArrowDown"),
        "ArrowLeft" => Some("ArrowLeft"),
        "ArrowRight" => Some("ArrowRight"),
        _ => None,
    }
}

impl App {
    fn new(
        once: bool,
        exit_after_activation: bool,
        font: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let source = scene();
        let mut engine = HostRenderer::new(SCENE_OWNER, &source, font, 640, 360)
            .ok_or("native scene rejected")?;
        let generation = engine
            .surface_status(SCENE_OWNER)
            .ok_or("native surface missing")?
            .generation;
        if !engine.focus(
            SCENE_OWNER,
            source.version(),
            generation,
            BUTTON,
            &mut |_| {},
        ) {
            return Err("initial scene focus rejected".into());
        }

        let context = UiInputContext {
            target: TEXT_TARGET,
            target_generation: 1,
            document: 20,
            revision: 1,
        };
        let text = TextDisplayView::new(TEXT_OWNER, context, font, "External display", 320, 120);
        let mut text_binding = UiInputBinding::new(TEXT_OWNER, 1);
        let ticket = text_binding
            .bind(TEXT_OWNER, context)
            .ok_or("initial text binding rejected")?;
        let mut text = text;
        if !text.focus(&text_binding, TEXT_OWNER, ticket, &mut |_| {}) {
            return Err("initial text focus rejected".into());
        }

        Ok(Self {
            window: None,
            surface: None,
            engine,
            generation,
            viewport: None,
            backend: HeadlessBackend::new(1, 1),
            scene: source,
            input: DesktopInput::new(),
            text,
            text_binding,
            text_context: context,
            text_ticket: Some(ticket),
            text_preedit: None,
            text_snapshot: "External display".into(),
            external_dirty: true,
            scroll_y: 0.0,
            activated: 0,
            once,
            exit_after_activation,
            presentation_error: None,
            closed: false,
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
        let viewport = (size.width, size.height, window.scale_factor() as f32);
        if self.viewport != Some(viewport) {
            self.generation = self
                .engine
                .reconfigure(10, self.generation, viewport.0, viewport.1, viewport.2)
                .map_err(|error| format!("native viewport rejected: {error:?}"))?;
            self.viewport = Some(viewport);
        }
        if !self
            .text
            .set_scale(TEXT_OWNER, self.text_context, window.scale_factor() as f32)
            || !self
                .text
                .resize(TEXT_OWNER, self.text_context, size.width, size.height)
        {
            return Err("external text viewport rejected".into());
        }
        if self.external_dirty {
            if self
                .text
                .render_cpu(TEXT_OWNER, self.text_context, BUDGET)
                .is_none()
            {
                return Err("external text frame rejected".into());
            }
            self.external_dirty = false;
        }

        self.engine
            .render_to_backend(
                SCENE_OWNER,
                self.scene.version(),
                self.generation,
                &mut self.backend,
                BUDGET,
            )
            .map_err(|error| format!("native CPU frame rejected: {error:?}"))?;
        let pixels = self.backend.canvas().pixels();
        let mut buffer = surface.buffer_mut()?;
        rgba_to_u32(pixels, &mut buffer)
            .map_err(|error| format!("pixel conversion failed: {error:?}"))?;
        buffer.present()?;
        Ok(())
    }

    fn close_text_focus(&mut self) -> bool {
        let Some(ticket) = self.text_ticket.take() else {
            return true;
        };
        if let Some(token) = self.text_preedit.take() {
            self.text.cancel_preedit(TEXT_OWNER, token);
        }
        if self
            .text
            .close_focus(&mut self.text_binding, TEXT_OWNER, ticket, &mut |_| {})
        {
            return true;
        }
        self.text_binding.retire();
        false
    }

    fn restore_text_focus(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.text_ticket.is_some() {
            return Ok(());
        }
        let ticket = self
            .text_binding
            .bind(TEXT_OWNER, self.text_context)
            .ok_or("text focus rebind rejected")?;
        if !self
            .text
            .focus(&self.text_binding, TEXT_OWNER, ticket, &mut |_| {})
        {
            return Err("text focus restore rejected".into());
        }
        self.text_ticket = Some(ticket);
        Ok(())
    }

    fn service_snapshot(&mut self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        let ticket = self.text_ticket.ok_or("missing text focus ticket")?;
        if let Some(token) = self.text_preedit.take() {
            if !self.text.cancel_preedit(TEXT_OWNER, token) {
                return Err("preedit cancellation before commit rejected".into());
            }
        }
        let next = UiInputContext {
            revision: self
                .text_context
                .revision
                .checked_add(1)
                .ok_or("external text revision exhausted")?,
            ..self.text_context
        };
        if !self
            .text
            .snapshot(TEXT_OWNER, self.text_context, next, &text)
        {
            return Err("external service snapshot rejected".into());
        }
        let successor = self
            .text_binding
            .replace(TEXT_OWNER, ticket, next)
            .ok_or("text binding rotation rejected")?;
        if !self
            .text
            .focus(&self.text_binding, TEXT_OWNER, successor, &mut |_| {})
        {
            return Err("successor text focus rejected".into());
        }
        self.text_context = next;
        self.text_ticket = Some(successor);
        self.text_snapshot = text;
        self.external_dirty = true;
        Ok(())
    }

    fn update_preedit(&mut self, text: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        if text.is_none_or(str::is_empty) {
            if let Some(token) = self.text_preedit.take() {
                if !self.text.cancel_preedit(TEXT_OWNER, token) {
                    return Err("preedit cancel rejected".into());
                }
            }
            return Ok(());
        }
        if self.text_preedit.is_none() {
            let ticket = self.text_ticket.ok_or("missing text focus ticket")?;
            self.text_preedit = Some(
                self.text
                    .begin_preedit(&self.text_binding, TEXT_OWNER, ticket)
                    .ok_or("preedit begin rejected")?,
            );
        }
        let token = self.text_preedit.ok_or("missing preedit token")?;
        if !self
            .text
            .preedit(&self.text_binding, TEXT_OWNER, token, text)
        {
            return Err("preedit update rejected".into());
        }
        self.external_dirty = true;
        Ok(())
    }

    fn apply_visual_switch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Windows supplies a single semantic activation after pointer-up. The pinned native
        // click default can clear focus, so the embedding host applies its retained navigation
        // intent before the visual frame; no intermediate unfocused frame is presented.
        if self.engine.focused_node(SCENE_OWNER, self.scene.version()) != Some(BUTTON)
            && !self.engine.focus(
                SCENE_OWNER,
                self.scene.version(),
                self.generation,
                BUTTON,
                &mut |_| {},
            )
        {
            return Err("visual switch focus restore rejected".into());
        }
        let old = self.scene.version();
        let background = if self.activated % 2 == 0 {
            Color::rgb(48, 160, 105)
        } else {
            Color::rgb(40, 130, 90)
        };
        let patches = [ScenePatch::UpdateStyle {
            id: BUTTON,
            style: Style::default()
                .size(Length::Px(200.0), Length::Px(52.0))
                .background(background)
                .font(700, FontStyle::Normal),
        }];
        self.scene
            .apply_batch_at(old, &patches)
            .map_err(|error| format!("source visual update rejected: {error:?}"))?;
        if !self.engine.apply_with_focus_events(
            SCENE_OWNER,
            old,
            self.scene.version(),
            self.generation,
            &patches,
            &mut |_| panic!("visual switch changed scene focus"),
        ) {
            return Err("native visual update rejected".into());
        }
        if self.engine.focused_node(SCENE_OWNER, self.scene.version()) != Some(BUTTON) {
            return Err("visual switch lost scene focus".into());
        }
        let accessibility = self
            .engine
            .accessibility_tree(SCENE_OWNER, self.scene.version(), self.generation)
            .ok_or("visual switch accessibility tree rejected")?;
        if accessibility.focused != Some(BUTTON)
            || !accessibility
                .nodes
                .iter()
                .any(|node| node.node == BUTTON && node.actions.activate)
        {
            return Err("visual switch lost accessibility state".into());
        }
        Ok(())
    }

    fn dispatch_input(&mut self, event: InputEvent) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            InputEvent::Pointer(pointer) => {
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
                let pressed = matches!(pointer.phase, PointerPhase::Down | PointerPhase::Move);
                let mut activation = false;
                if !self.engine.pointer(
                    SCENE_OWNER,
                    self.scene.version(),
                    self.generation,
                    action,
                    pointer.position.x / scale,
                    pointer.position.y / scale,
                    pressed,
                    &mut |event| {
                        activation |=
                            event.kind == jasper_ui_engine::native::SceneEventKind::Activate;
                        EventReply::default()
                    },
                    &mut |_| {},
                ) {
                    return Err("scene pointer event rejected".into());
                }
                if activation {
                    self.activated += 1;
                    self.apply_visual_switch()?;
                }
            }
            InputEvent::Scroll(scroll) => {
                self.scroll_y = (self.scroll_y + scroll.delta.y as f64).clamp(0.0, 32.0);
                if !self.engine.set_viewport_scroll(
                    SCENE_OWNER,
                    self.scene.version(),
                    self.generation,
                    0.0,
                    self.scroll_y,
                ) {
                    return Err("scene viewport scroll rejected".into());
                }
            }
            InputEvent::Key(key) => {
                let Some(code) = command_code(&key.key) else {
                    return Ok(());
                };
                let native = HostKeyInput {
                    key: &key.key,
                    code,
                    pressed: key.phase == KeyPhase::Down,
                    repeat: key.repeat,
                    composing: false,
                    modifiers: key.modifiers,
                    location: KeyLocation::Standard,
                };
                if !self.engine.key(
                    SCENE_OWNER,
                    self.scene.version(),
                    self.generation,
                    &native,
                    &mut |_| EventReply::default(),
                    &mut |_| {},
                ) {
                    return Err("scene command key rejected".into());
                }
            }
            InputEvent::Preedit { text, .. } => self.update_preedit(Some(&text))?,
            InputEvent::Text(text) => {
                self.text_snapshot.push_str(&text);
                let snapshot = self.text_snapshot.clone();
                self.service_snapshot(snapshot)?;
            }
            InputEvent::Composition(_) => {}
            InputEvent::ImeEnabled(enabled) => {
                if !enabled {
                    self.update_preedit(None)?;
                }
            }
            InputEvent::Focus(focus) => {
                if focus.focused {
                    if !self.engine.focus(
                        SCENE_OWNER,
                        self.scene.version(),
                        self.generation,
                        BUTTON,
                        &mut |_| {},
                    ) {
                        return Err("scene focus restore rejected".into());
                    }
                    self.restore_text_focus()?;
                } else {
                    if !self.engine.deactivate(
                        SCENE_OWNER,
                        self.scene.version(),
                        self.generation,
                        &mut |_| {},
                    ) {
                        return Err("scene deactivation rejected".into());
                    }
                    if !self.close_text_focus() {
                        return Err("text focus loss rejected".into());
                    }
                }
            }
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        if self.closed {
            return;
        }
        let version = self.scene.version();
        if let Some(token) = self.text_preedit.take() {
            self.text.cancel_preedit(TEXT_OWNER, token);
        }
        self.close_text_focus();
        self.text.close(TEXT_OWNER, self.text_context);
        self.engine
            .deactivate(SCENE_OWNER, version, self.generation, &mut |_| {});
        match self
            .engine
            .retire(SCENE_OWNER, version, self.generation, &mut |_| {})
        {
            Ok(()) => self.closed = true,
            Err(error) => {
                self.presentation_error = Some(format!("native retirement rejected: {error:?}"));
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.closed {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let attributes = Window::default_attributes().with_title("Jasper UI Engine");
            let window = Arc::new(event_loop.create_window(attributes)?);
            let size = window.inner_size();
            let scale = window.scale_factor() as f32;
            let state = self
                .engine
                .surface_status(SCENE_OWNER)
                .ok_or("native surface missing")?;
            if state.state == SurfaceState::Suspended {
                self.generation = self.engine.resume(
                    SCENE_OWNER,
                    self.generation,
                    size.width.max(1),
                    size.height.max(1),
                    scale,
                )?;
            }
            self.viewport = None;
            let context = Context::new(window.clone())?;
            let surface = Surface::new(&context, window.clone())?;
            self.engine.focus(
                SCENE_OWNER,
                self.scene.version(),
                self.generation,
                BUTTON,
                &mut |_| {},
            );
            self.restore_text_focus()?;
            self.window = Some(window.clone());
            self.surface = Some(surface);
            window.request_redraw();
            Ok(())
        })();
        if let Err(error) = result {
            self.presentation_error = Some(error.to_string());
            event_loop.exit();
        }
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        match self.engine.suspend(
            SCENE_OWNER,
            self.scene.version(),
            self.generation,
            &mut |_| {},
        ) {
            Ok(generation) => self.generation = generation,
            Err(error) => {
                self.presentation_error = Some(format!("native suspend rejected: {error:?}"));
                event_loop.exit();
                return;
            }
        }
        self.close_text_focus();
        self.viewport = None;
        self.surface = None;
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
                self.shutdown();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.render() {
                    self.presentation_error = Some(error.to_string());
                    eprintln!("presentation failed: {error}");
                    event_loop.exit();
                    return;
                }
                if self.once || (self.exit_after_activation && self.activated > 0) {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            other => {
                let inputs = self.input.translate(&other);
                if inputs.is_empty() {
                    return;
                }
                for input in inputs {
                    if let Err(error) = self.dispatch_input(input) {
                        self.presentation_error = Some(error.to_string());
                        eprintln!("input failed: {error}");
                        event_loop.exit();
                        return;
                    }
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
    let (once, exit_after_activation) = match arguments.next().as_deref() {
        None => (false, false),
        Some("--once") => (true, false),
        Some("--once-after-activation") => (false, true),
        _ => return Err("expected FONT [--once|--once-after-activation]".into()),
    };
    if arguments.next().is_some() {
        return Err("expected FONT [--once|--once-after-activation]".into());
    }
    let font = std::fs::read(font_path)?;
    let event_loop = EventLoop::new()?;
    let mut app = App::new(once, exit_after_activation, &font)?;
    let result = event_loop.run_app(&mut app);
    app.shutdown();
    result?;
    if let Some(error) = app.presentation_error {
        return Err(error.into());
    }
    println!(
        "item5_window=pass activated={} scene={}x{} external_snapshot_revision={} retired={}",
        app.activated, 640, 360, app.text_context.revision, app.closed
    );
    Ok(())
}
