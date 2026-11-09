//! Linux/desktop reference host with two independent native UI windows.
//! Linux/桌面双窗口参考宿主；每个窗口拥有独立场景、Surface 代数与原生引擎。
//!
//! Engine state is separated from host presentation objects so a session suspend can release
//! Window/Surface resources without discarding per-window scenes. Other platform adapters can
//! create the same per-window owners without depending on Wayland or X11.
//! 引擎状态与宿主呈现对象分离，会话暂停时释放 Window/Surface 而不丢弃各窗口场景；其他平台
//! 适配器可以用同样的每窗口 owner 接入，而不依赖 Wayland 或 X11。
//!
//! Run:
//! `cargo run --locked --features desktop,native-engine-cpu --example multiwindow -- FONT [--once]`

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::desktop::rgba_to_u32;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::host::{HostRenderer, SurfaceState};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const BUDGET: usize = 64 * 1024 * 1024;
const OWNERS: [u64; 2] = [100, 101];

fn scene(owner: u64) -> Scene {
    let accent = if owner == OWNERS[0] {
        Color::RED
    } else {
        Color::BLUE
    };
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .background(Color::new(24, 26, 32, 255))
                    .padding(Edges::all(32.0))
                    .gap(24.0)
                    .direction(FlexDirection::Row),
            )
            .child(
                Node::rect(NodeId(2)).with_style(
                    Style::default()
                        .size(Length::Px(200.0), Length::Px(120.0))
                        .background(accent),
                ),
            ),
    )
}

/// Scene/engine state that survives host window and Surface replacement.
/// 在宿主窗口与 Surface 替换后仍保留的场景/引擎状态。
struct ViewState {
    owner: u64,
    scene: Scene,
    engine: HostRenderer,
    generation: u64,
    rendered: bool,
}

impl ViewState {
    fn new(owner: u64, font: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let scene = scene(owner);
        let engine =
            HostRenderer::new(owner, &scene, font, 640, 360).ok_or("native scene rejected")?;
        let generation = engine
            .surface_status(owner)
            .ok_or("native surface missing")?
            .generation;
        Ok(Self {
            owner,
            scene,
            engine,
            generation,
            rendered: false,
        })
    }

    fn activate(
        &mut self,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let status = self
            .engine
            .surface_status(self.owner)
            .ok_or("native surface missing")?;
        self.generation = if status.state == SurfaceState::Suspended {
            self.engine
                .resume(self.owner, self.generation, width, height, scale)?
        } else if status.width != width || status.height != height {
            self.engine
                .reconfigure(self.owner, self.generation, width, height, scale)?
        } else {
            self.generation
        };
        self.rendered = false;
        Ok(())
    }

    fn suspend(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let status = self
            .engine
            .surface_status(self.owner)
            .ok_or("native surface missing")?;
        if status.state == SurfaceState::Active {
            self.generation = self.engine.suspend(
                self.owner,
                self.scene.version(),
                self.generation,
                &mut |_| {},
            )?;
        }
        self.rendered = false;
        Ok(())
    }

    fn retire(&mut self) {
        let _ = self.engine.retire(
            self.owner,
            self.scene.version(),
            self.generation,
            &mut |_| {},
        );
    }
}

/// Host-owned window and presentation resources.
/// 宿主持有的窗口与呈现资源。
struct PresentedWindow {
    owner: u64,
    window: Arc<Window>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    backend: HeadlessBackend,
    viewport: Option<(u32, u32, f32)>,
}

impl PresentedWindow {
    fn new(owner: u64, window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new(window.clone())?;
        let surface = Surface::new(&context, window.clone())?;
        Ok(Self {
            owner,
            window,
            surface,
            backend: HeadlessBackend::new(1, 1),
            viewport: None,
        })
    }

    fn render(&mut self, state: &mut ViewState) -> Result<(), Box<dyn std::error::Error>> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        self.surface.resize(width, height)?;
        let viewport = (size.width, size.height, self.window.scale_factor() as f32);
        if self.viewport != Some(viewport) {
            state.generation = state.engine.reconfigure(
                state.owner,
                state.generation,
                viewport.0,
                viewport.1,
                viewport.2,
            )?;
            self.viewport = Some(viewport);
        }
        state
            .engine
            .render_to_backend(
                state.owner,
                state.scene.version(),
                state.generation,
                &mut self.backend,
                BUDGET,
            )
            .map_err(|error| format!("native CPU frame rejected: {error:?}"))?;
        let mut buffer = self.surface.buffer_mut()?;
        rgba_to_u32(self.backend.canvas().pixels(), &mut buffer)
            .map_err(|error| format!("pixel conversion failed: {error:?}"))?;
        buffer.present()?;
        Ok(())
    }
}

struct MultiWindowApp {
    states: BTreeMap<u64, ViewState>,
    presented: BTreeMap<WindowId, PresentedWindow>,
    font: Vec<u8>,
    once: bool,
    exercise_suspend_resume: bool,
    suspend_resume_cycle: bool,
    presentation_error: Option<String>,
}

impl MultiWindowApp {
    fn new(font: &[u8], once: bool, exercise_suspend_resume: bool) -> Self {
        Self {
            states: BTreeMap::new(),
            presented: BTreeMap::new(),
            font: font.to_vec(),
            once,
            exercise_suspend_resume,
            suspend_resume_cycle: false,
            presentation_error: None,
        }
    }

    fn create_states(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for owner in OWNERS {
            self.states
                .insert(owner, ViewState::new(owner, &self.font)?);
        }
        Ok(())
    }

    fn present_windows(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (index, owner) in OWNERS.into_iter().enumerate() {
            let title = format!("Jasper UI Engine Window {}", index + 1);
            let window =
                Arc::new(event_loop.create_window(Window::default_attributes().with_title(title))?);
            let size = window.inner_size();
            let scale = window.scale_factor() as f32;
            let state = self.states.get_mut(&owner).ok_or("missing window state")?;
            state.activate(size.width.max(1), size.height.max(1), scale)?;
            let id = window.id();
            let presented = PresentedWindow::new(owner, window)?;
            presented.window.request_redraw();
            self.presented.insert(id, presented);
        }
        Ok(())
    }

    fn suspend_states(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for state in self.states.values_mut() {
            state.suspend()?;
        }
        self.presented.clear();
        Ok(())
    }

    fn retire_all(&mut self) {
        for state in self.states.values_mut() {
            state.retire();
        }
        self.states.clear();
        self.presented.clear();
    }
}

impl ApplicationHandler for MultiWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.presented.is_empty() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if self.states.is_empty() {
                self.create_states()?;
            }
            self.present_windows(event_loop)
        })();
        if let Err(error) = result {
            self.presentation_error = Some(error.to_string());
            event_loop.exit();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Err(error) = self.suspend_states() {
            self.presentation_error = Some(error.to_string());
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                let Some(presented) = self.presented.remove(&window_id) else {
                    return;
                };
                let owner = presented.owner;
                if let Some(mut state) = self.states.remove(&owner) {
                    state.retire();
                }
                if self.states.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(presented) = self.presented.get(&window_id) {
                    presented.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let result = {
                    let Some(presented) = self.presented.get_mut(&window_id) else {
                        return;
                    };
                    let owner = presented.owner;
                    let Some(state) = self.states.get_mut(&owner) else {
                        return;
                    };
                    presented.render(state).map(|()| {
                        state.rendered = true;
                        owner
                    })
                };
                let owner = match result {
                    Ok(owner) => owner,
                    Err(error) => {
                        self.presentation_error = Some(format!(
                            "window owner {window_id:?} presentation failed: {error}"
                        ));
                        event_loop.exit();
                        return;
                    }
                };
                if !self.states.values().all(|state| state.rendered) {
                    return;
                }
                if self.exercise_suspend_resume && !self.suspend_resume_cycle {
                    self.suspend_resume_cycle = true;
                    if let Err(error) = self.suspend_states() {
                        self.presentation_error = Some(error.to_string());
                        event_loop.exit();
                        return;
                    }
                    for state in self.states.values_mut() {
                        state.rendered = false;
                    }
                    if let Err(error) = self.present_windows(event_loop) {
                        self.presentation_error = Some(error.to_string());
                        event_loop.exit();
                        return;
                    }
                    return;
                }
                if self.once {
                    eprintln!("multiwindow owner {owner} completed final presentation");
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
    let mut once = false;
    let mut exercise_suspend_resume = false;
    for argument in arguments {
        match argument.as_str() {
            "--once" => once = true,
            "--exercise-suspend-resume" => exercise_suspend_resume = true,
            _ => return Err("expected FONT [--once] [--exercise-suspend-resume]".into()),
        }
    }
    let font = std::fs::read(font_path)?;
    let event_loop = EventLoop::new()?;
    let mut app = MultiWindowApp::new(&font, once, exercise_suspend_resume);
    let result = event_loop.run_app(&mut app);
    app.retire_all();
    result?;
    if let Some(error) = app.presentation_error {
        return Err(error.into());
    }
    if app.once {
        println!(
            "multiwindow=pass owners={:?} engine_state_separated=true suspend_resume_exercised={}",
            OWNERS, app.suspend_resume_cycle
        );
    }
    Ok(())
}
