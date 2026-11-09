//! iOS / iPadOS reference host for the Jasper UI native engine.
//! Jasper UI 原生引擎的 iOS / iPadOS 参考宿主。
//!
//! The host owns the UIKit window, the `CAMetalLayer` surface, the wgpu Metal device/queue and
//! the UIKit lifecycle. The engine stays host-neutral and receives only owned scene values,
//! normalized touch/keyboard events and surface generations.
//!
//! 宿主持有 UIKit 窗口、`CAMetalLayer` Surface、wgpu Metal 设备/队列与 UIKit 生命周期；
//! 引擎保持宿主中立，只接收自有场景值、规范化触摸/键盘事件与 Surface 代数。
//!
//! iPad uses the same binary: `UIDeviceFamily` in the bundle covers iPhone and iPad, and the
//! viewport model applies safe-area insets supplied by the host adapter.
//! iPad 使用同一二进制：bundle 的 `UIDeviceFamily` 覆盖 iPhone 与 iPad，
//! viewport 模型应用宿主适配器给出的安全区内缩。
//!
//! Run (device or simulator): `jasper-ui FONT [--frames N] [--readback out.ppm]
//! [--insets top,right,bottom,left] [--timeout-ms N] [--tap X,Y]`.
//! This is a reference platform adapter, not an application framework.

mod model;

#[cfg(all(not(target_os = "ios"), not(feature = "host-on-macos")))]
fn main() {
    eprintln!("jasper-ui iOS host builds for aarch64-apple-ios and aarch64-apple-ios-sim");
}

/// `host-on-macos` compiles the identical adapter for a macOS build so its window, surface,
/// viewport, scene and input logic can be exercised without an iOS Simulator runtime. It is not
/// an iOS runtime and does not cover UIKit lifecycle or device metrics.
/// `host-on-macos` 把同一份适配器编译到 macOS 构建，以便在没有 iOS 模拟器运行时的机器上
/// 验证窗口、Surface、viewport、场景与输入逻辑；它不是 iOS 运行时，也不覆盖 UIKit 生命周期或真机指标。
#[cfg(any(target_os = "ios", feature = "host-on-macos"))]
mod host {
    use std::num::NonZeroU32;
    use std::sync::Arc;
    use std::time::Instant;

    use jasper_ui_engine::desktop::DesktopInput;
    use jasper_ui_engine::geometry::Edges;
    use jasper_ui_engine::input::{InputEvent, PointerPhase};
    use jasper_ui_engine::native::host::{GpuRenderer, HostRenderer, SurfaceState};
    use jasper_ui_engine::native::{EventReply, PointerAction};
    use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
    use jasper_ui_engine::semantics::{Role, Semantics};
    use jasper_ui_engine::style::{Align, Color, FlexDirection, Justify, Length, Style};
    use winit::application::ApplicationHandler;
    use winit::event::{StartCause, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::{Window, WindowId};

    use crate::model::{
        Content, Insets, Lifecycle, LifecycleAction, PrimaryContact, Viewport,
    };

    const OWNER: u64 = 0x4a53_5045_522d_494f; // "JASPER-IO"
    const BUTTON: NodeId = NodeId(2);
    const BUDGET: usize = 64 * 1024 * 1024;
    /// Host-injected ordered font stack: Latin first, CJK fallback second.
    /// 宿主注入的有序字体栈：拉丁在前、CJK 回退在后。
    const LATIN: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");
    const CJK: &[u8] = include_bytes!("../assets/DroidSansFallbackFull.ttf");

    const BACKGROUND: Color = Color::rgb(18, 20, 26);
    const ACTIVE: Color = Color::rgb(40, 130, 90);
    const PRESSED: Color = Color::rgb(190, 92, 52);

    /// Machine-readable progress line; the on-device console and `simctl` logs consume it.
    /// 机器可读的进度行；真机控制台与 `simctl` 日志据此匹配。
    fn stage(name: &str, detail: &str) {
        eprintln!("stage={name} {detail}");
    }

    struct Options {
        frames: Option<u64>,
        readback: Option<String>,
        insets: Insets,
        timeout_ms: Option<u64>,
        tap: Option<(f32, f32)>,
        /// Tap the scene button's accessibility bounds instead of fixed logical coordinates.
        /// 按场景按钮的无障碍 bounds 点击，而不是固定逻辑坐标。
        tap_center: bool,
        /// Keep rendering until the frame target is reached instead of waiting for events.
        /// 持续渲染直到达到帧目标，而不是等待事件。
        spin: bool,
        /// Logical window size for the macOS adapter exercise; UIKit ignores it on device.
        /// macOS 适配器演练用的逻辑窗口尺寸；真机上 UIKit 会忽略它。
        size: Option<(f32, f32)>,
        /// Optional CJK font file injected after the Latin font by the host.
        /// 宿主在拉丁字体之后注入的可选中日韩字体文件。
        cjk_font: Option<String>,
        /// Add the Chinese line without a CJK font, to measure the fallback difference.
        /// 只加中文行、不注入中日韩字体，用于测量回退差异。
        cjk_tofu: bool,
    }

    fn parse_args() -> Result<(Options, Vec<u8>), String> {
        let mut font_path = None;
        let mut options = Options {
            frames: None,
            readback: None,
            insets: Insets::default(),
            timeout_ms: None,
            tap: None,
            tap_center: false,
            spin: false,
            size: None,
            cjk_font: None,
            cjk_tofu: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--frames" => {
                    let value = args.next().ok_or("--frames needs a count")?;
                    options.frames = Some(value.parse().map_err(|_| "invalid --frames count")?);
                }
                "--readback" => {
                    options.readback = Some(args.next().ok_or("--readback needs a path")?);
                }
                "--insets" => {
                    let value = args.next().ok_or("--insets needs top,right,bottom,left")?;
                    let parts: Vec<f32> = value
                        .split(',')
                        .map(|part| part.parse::<f32>())
                        .collect::<Result<_, _>>()
                        .map_err(|_| "invalid --insets value")?;
                    if parts.len() != 4 {
                        return Err("--insets needs four comma-separated values".into());
                    }
                    options.insets = Insets::new(parts[0], parts[1], parts[2], parts[3]);
                }
                "--timeout-ms" => {
                    let value = args.next().ok_or("--timeout-ms needs a number")?;
                    options.timeout_ms = Some(value.parse().map_err(|_| "invalid --timeout-ms")?);
                }
                "--cjk-font" => {
                    options.cjk_font = Some(args.next().ok_or("--cjk-font needs a path")?);
                }
                "--cjk-tofu" => options.cjk_tofu = true,
                "--size" => {
                    let value = args.next().ok_or("--size needs W,H")?;
                    let parts: Vec<f32> = value
                        .split(',')
                        .map(|part| part.parse::<f32>())
                        .collect::<Result<_, _>>()
                        .map_err(|_| "invalid --size value")?;
                    if parts.len() != 2 {
                        return Err("--size needs W,H".into());
                    }
                    options.size = Some((parts[0], parts[1]));
                }
                "--spin" => options.spin = true,
                "--tap" => {
                    let value = args.next().ok_or("--tap needs X,Y or center")?;
                    if value == "center" {
                        options.tap_center = true;
                        continue;
                    }
                    let parts: Vec<f32> = value
                        .split(',')
                        .map(|part| part.parse::<f32>())
                        .collect::<Result<_, _>>()
                        .map_err(|_| "invalid --tap value")?;
                    if parts.len() != 2 {
                        return Err("--tap needs X,Y".into());
                    }
                    options.tap = Some((parts[0], parts[1]));
                }
                other => {
                    if other.starts_with("--") {
                        return Err(format!("unknown flag {other}"));
                    }
                    if font_path.replace(other.to_string()).is_some() {
                        return Err("more than one font path".into());
                    }
                }
            }
        }
        let font_path = font_path.ok_or("usage: jasper-ui FONT [options]")?;
        let font =
            std::fs::read(&font_path).map_err(|error| format!("font {font_path}: {error}"))?;
        Ok((options, font))
    }

    fn scene(chinese: bool) -> Scene {
        Scene::new(
            Node::container(NodeId(1))
                .with_style(
                    Style::default()
                        .width(Length::Percent(100.0))
                        .height(Length::Percent(100.0))
                        .padding(Edges::all(24.0))
                        .gap(16.0)
                        .direction(FlexDirection::Column)
                        .justify_content(Justify::Center)
                        .align_items(Align::Center)
                        .background(BACKGROUND),
                )
                .child(
                    Node::text(NodeId(3), "Jasper UI · iPad / iOS").with_style(
                        Style::default()
                            .background(BACKGROUND)
                            .size(Length::Percent(80.0), Length::Px(32.0))
                            .text(24.0, Color::rgb(230, 234, 240)),
                    ),
                )
                .child(if chinese {
                    Node::text(NodeId(4), "中文渲染正常，无乱码：昆斤考烫烫烫").with_style(
                        Style::default()
                            .background(BACKGROUND)
                            .size(Length::Percent(80.0), Length::Px(28.0))
                            .text(20.0, Color::rgb(230, 234, 240)),
                    )
                } else {
                    Node::text(NodeId(4), "UTF-8 text check").with_style(
                        Style::default()
                            .background(BACKGROUND)
                            .size(Length::Percent(80.0), Length::Px(28.0))
                            .text(20.0, Color::rgb(230, 234, 240)),
                    )
                })
                .child(
                    Node::text(BUTTON, "激活 Activate")
                        .with_semantics(
                            Semantics::default()
                                .role(Role::Button)
                                .label("激活 Activate")
                                .focusable(true),
                        )
                        .with_style(
                            Style::default()
                                .width(Length::Percent(80.0))
                                .height(Length::Px(160.0))
                                .background(ACTIVE)
                                .text(32.0, Color::rgb(245, 247, 250)),
                        ),
                ),
        )
    }

    fn root_style(content: Content) -> Style {
        Style::default()
            .width(Length::Px(content.width))
            .height(Length::Px(content.height))
            .padding(Edges::all(24.0))
            .gap(16.0)
            .direction(FlexDirection::Column)
            .justify_content(Justify::Center)
            .align_items(Align::Center)
            .background(BACKGROUND)
    }

    struct FrameRecord {
        path: String,
        bytes: usize,
        background: usize,
        active: usize,
        pressed: usize,
        other: usize,
        sha256: String,
    }

    struct App {
        options: Options,
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
        contacts: PrimaryContact,
        lifecycle: Lifecycle,
        viewport: Option<Viewport>,
        activations: u64,
        frames: u64,
        suspended: bool,
        closed: bool,
        started: Instant,
        tapped: bool,
        records: Vec<FrameRecord>,
        error: Option<String>,
    }

    impl App {
        fn new(
            options: Options,
            latin: &[u8],
            cjk: Option<&[u8]>,
        ) -> Result<Self, String> {
            let chinese = !options.cjk_tofu;
            let source = scene(chinese);
            // `--cjk-tofu` deliberately drops the fallback face to measure the difference.
            // `--cjk-tofu` 故意去掉回退字面，用于测量差异。
            let stack: Vec<&[u8]> = if options.cjk_tofu {
                vec![latin]
            } else {
                vec![latin, cjk.unwrap_or(CJK)]
            };
            // The initial extent is replaced by the first real viewport in `resume`.
            // 初始尺寸会被 `resume` 中的真实 viewport 替换。
            let engine = HostRenderer::with_fonts(OWNER, &source, &stack, 390, 844)
                .ok_or("native scene import rejected")?;
            let generation = engine
                .surface_status(OWNER)
                .ok_or("native surface missing")?
                .generation;
            Ok(Self {
                options,
                window: None,
                surface: None,
                config: None,
                device: None,
                queue: None,
                gpu: None,
                engine,
                generation,
                scene: source,
                input: DesktopInput::new(),
                contacts: PrimaryContact::new(),
                lifecycle: Lifecycle::new(),
                viewport: None,
                activations: 0,
                frames: 0,
                suspended: false,
                closed: false,
                started: Instant::now(),
                tapped: false,
                records: Vec::new(),
                error: None,
            })
        }

        fn apply_viewport(&mut self) -> Result<(), String> {
            let window = self.window.clone().ok_or("missing UIKit window")?;
            let physical = window.inner_size();
            let scale = window.scale_factor() as f32;
            let viewport = Viewport::new(
                physical.width.max(1),
                physical.height.max(1),
                scale,
                self.options.insets,
            );
            if self.viewport == Some(viewport) {
                return Ok(());
            }
            self.viewport = Some(viewport);
            self.generation = self
                .engine
                .reconfigure(
                    OWNER,
                    self.generation,
                    viewport.physical_width,
                    viewport.physical_height,
                    viewport.scale,
                )
                .map_err(|error| format!("native viewport rejected: {error:?}"))?;
            let old = self.scene.version();
            let patch = ScenePatch::UpdateStyle {
                id: NodeId(1),
                style: root_style(viewport.content()),
            };
            self.scene
                .apply_batch_at(old, std::slice::from_ref(&patch))
                .map_err(|error| format!("iOS root resize rejected: {error:?}"))?;
            if !self.engine.apply_with_focus_events(
                OWNER,
                old,
                self.scene.version(),
                self.generation,
                std::slice::from_ref(&patch),
                &mut |_| {},
            ) {
                return Err("native iOS root resize rejected".into());
            }
            let content = viewport.content();
            stage(
                "viewport",
                &format!(
                    "physical={}x{} scale={} logical={:.1}x{:.1} content={:.1}x{:.1}@{:.1},{:.1} generation={}",
                    viewport.physical_width,
                    viewport.physical_height,
                    viewport.scale,
                    viewport.logical_size().0,
                    viewport.logical_size().1,
                    content.width,
                    content.height,
                    content.x,
                    content.y,
                    self.generation
                ),
            );
            Ok(())
        }

        fn activate(&mut self) -> Result<(), String> {
            self.activations += 1;
            let background = if self.activations % 2 == 0 {
                ACTIVE
            } else {
                PRESSED
            };
            let old = self.scene.version();
            let patch = ScenePatch::UpdateStyle {
                id: BUTTON,
                style: Style::default()
                    .width(Length::Percent(80.0))
                    .height(Length::Px(160.0))
                    .background(background)
                    .text(32.0, Color::rgb(245, 247, 250)),
            };
            self.scene
                .apply_batch_at(old, std::slice::from_ref(&patch))
                .map_err(|error| format!("iOS source update rejected: {error:?}"))?;
            if !self.engine.apply_with_focus_events(
                OWNER,
                old,
                self.scene.version(),
                self.generation,
                std::slice::from_ref(&patch),
                &mut |_| {},
            ) {
                return Err("native iOS style update rejected".into());
            }
            stage(
                "activate",
                &format!("count={} version={}", self.activations, self.scene.version()),
            );
            Ok(())
        }

        fn button_center(&self) -> Option<(f32, f32)> {
            let tree = self
                .engine
                .accessibility_tree(OWNER, self.scene.version(), self.generation)?;
            let node = tree.nodes.iter().find(|node| node.node == BUTTON)?;
            let bounds = node.bounds?;
            Some((
                bounds.origin.x + bounds.size.width / 2.0,
                bounds.origin.y + bounds.size.height / 2.0,
            ))
        }

        fn tap(&mut self, x: f32, y: f32) -> Result<bool, String> {
            let mut activated = false;
            for (action, pressed) in [(PointerAction::Down, true), (PointerAction::Up, false)] {
                if !self.engine.pointer(
                    OWNER,
                    self.scene.version(),
                    self.generation,
                    action,
                    x,
                    y,
                    pressed,
                    &mut |event| {
                        activated |= event.kind
                            == jasper_ui_engine::native::SceneEventKind::Activate;
                        EventReply::default()
                    },
                    &mut |_| {},
                ) {
                    return Err("native iOS pointer rejected".into());
                }
            }
            if activated {
                self.activate()?;
            }
            Ok(activated)
        }

        fn dispatch_inputs(&mut self, inputs: Vec<InputEvent>) -> Result<(), String> {
            let viewport = self.viewport.ok_or("missing iOS viewport")?;
            for input in inputs {
                match input {
                    InputEvent::Pointer(pointer) => {
                        let (x, y) = viewport.to_logical(pointer.position.x, pointer.position.y);
                        let action = match pointer.phase {
                            PointerPhase::Down => PointerAction::Down,
                            PointerPhase::Move | PointerPhase::Hover => PointerAction::Move,
                            PointerPhase::Up => PointerAction::Up,
                            PointerPhase::Cancel => PointerAction::Cancel,
                        };
                        match pointer.phase {
                            PointerPhase::Down => {
                                let primary = self.contacts.started(pointer.pointer_id.0);
                                stage(
                                    "contact",
                                    &format!(
                                        "id={} down primary={primary} active={}",
                                        pointer.pointer_id.0,
                                        self.contacts.active_count()
                                    ),
                                );
                            }
                            PointerPhase::Up | PointerPhase::Cancel => {
                                let was_primary = self.contacts.ended(pointer.pointer_id.0);
                                stage(
                                    "contact",
                                    &format!(
                                        "id={} up was_primary={was_primary} primary={:?} active={}",
                                        pointer.pointer_id.0,
                                        self.contacts.primary(),
                                        self.contacts.active_count()
                                    ),
                                );
                            }
                            PointerPhase::Move | PointerPhase::Hover => {}
                        }
                        let mut activated = false;
                        if !self.engine.pointer(
                            OWNER,
                            self.scene.version(),
                            self.generation,
                            action,
                            x,
                            y,
                            matches!(pointer.phase, PointerPhase::Down | PointerPhase::Move),
                            &mut |event| {
                                activated |= event.kind
                                    == jasper_ui_engine::native::SceneEventKind::Activate;
                                EventReply::default()
                            },
                            &mut |_| {},
                        ) {
                            return Err("native iOS pointer rejected".into());
                        }
                        if activated {
                            self.activate()?;
                        }
                    }
                    InputEvent::Focus(focus) if !focus.focused => {
                        if !self.engine.deactivate(
                            OWNER,
                            self.scene.version(),
                            self.generation,
                            &mut |_| {},
                        ) {
                            return Err("native iOS deactivation rejected".into());
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        }

        fn render(&mut self) -> Result<(), String> {
            if self.closed {
                return Ok(());
            }
            let window = self.window.clone().ok_or("missing UIKit window")?;
            let config = self.config.as_ref().ok_or("missing surface configuration")?;
            let format = config.format;
            let frame = match self
                .surface
                .as_ref()
                .ok_or("missing UIKit surface")?
                .get_current_texture()
            {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.configure_surface()?;
                    self.apply_viewport()?;
                    window.request_redraw();
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(())
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err("iOS surface validation failed".into())
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
                .map_err(|error| format!("iOS GPU frame rejected: {error:?}"))?;
            let (width, height) = (config.width, config.height);
            let readback = if let Some(path) = self.options.readback.clone() {
                Some(read_frame(
                    self.device.as_ref().ok_or("missing wgpu device")?,
                    self.queue.as_ref().ok_or("missing GPU queue")?,
                    &frame.texture,
                    format,
                    width,
                    height,
                    &path,
                )?)
            } else {
                None
            };
            self.queue
                .as_ref()
                .ok_or("missing GPU queue")?
                .present(frame);
            if !self.engine.gpu_presented(OWNER, self.generation) {
                return Err("iOS GPU present acknowledgement rejected".into());
            }
            self.frames += 1;
            match readback {
                Some(record) => {
                    stage(
                        "frame",
                        &format!(
                            "index={} {}x{} format={format:?} background={} active={} pressed={} other={} bytes={} sha256={}",
                            self.frames,
                            width,
                            height,
                            record.background,
                            record.active,
                            record.pressed,
                            record.other,
                            record.bytes,
                            record.sha256
                        ),
                    );
                    self.records.push(record);
                }
                None => stage(
                    "frame",
                    &format!("index={} {}x{} format={format:?}", self.frames, width, height),
                ),
            }
            if self.options.spin
                && self
                    .options
                    .frames
                    .is_some_and(|target| self.frames < target)
            {
                // Explicit diagnostic spin: keep presenting until the target is reached.
                // 显式诊断自旋：持续呈现直到达到目标帧数。
                window.request_redraw();
            }
            if !self.tapped && self.frames >= 1 {
                let target = if self.options.tap_center {
                    self.button_center()
                } else {
                    self.options.tap
                };
                if let Some((x, y)) = target {
                    self.tapped = true;
                    let activated = self.tap(x, y)?;
                    stage(
                        "tap",
                        &format!("x={x:.1} y={y:.1} activated={activated}"),
                    );
                    window.request_redraw();
                }
            }
            Ok(())
        }

        fn configure_surface(&mut self) -> Result<(), String> {
            let window = self.window.clone().ok_or("missing UIKit window")?;
            let physical = window.inner_size();
            let device = self.device.as_ref().ok_or("missing wgpu device")?;
            let config = self.config.as_mut().ok_or("missing surface configuration")?;
            let (width, height) = (physical.width.max(1), physical.height.max(1));
            if config.width == width && config.height == height {
                return Ok(());
            }
            config.width = width;
            config.height = height;
            self.surface
                .as_ref()
                .ok_or("missing UIKit surface")?
                .configure(device, config);
            Ok(())
        }

        fn shutdown(&mut self) {
            if self.closed {
                return;
            }
            let version = self.scene.version();
            let _ = self
                .engine
                .deactivate(OWNER, version, self.generation, &mut |_| {});
            match self
                .engine
                .retire(OWNER, version, self.generation, &mut |_| {})
            {
                Ok(()) => self.closed = true,
                Err(error) => {
                    self.error = Some(format!("native retirement rejected: {error:?}"));
                }
            }
        }

        fn teardown_surface(&mut self) {
            self.surface = None;
            self.config = None;
            self.device = None;
            self.queue = None;
            self.gpu = None;
            self.window = None;
        }

        fn summary(&self) -> String {
            let viewport = self
                .viewport
                .map(|viewport| {
                    format!(
                        "{}x{}@{}",
                        viewport.physical_width, viewport.physical_height, viewport.scale
                    )
                })
                .unwrap_or_else(|| "none".into());
            let adapter = self
                .gpu
                .as_ref()
                .map(|gpu| gpu.adapter_info())
                .unwrap_or_else(|| ("unknown".into(), "unknown".into()));
            format!(
                "ios_host={} adapter=\"{}\" backend={} viewport={} frames={} activations={} tapped={} retired={} elapsed_ms={}",
                if self.error.is_none() { "pass" } else { "fail" },
                adapter.0,
                adapter.1,
                viewport,
                self.frames,
                self.activations,
                self.tapped,
                self.closed,
                self.started.elapsed().as_millis()
            )
        }

        fn resume(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
            let mut attributes =
                Window::default_attributes().with_title("Jasper UI · iOS");
            if let Some((width, height)) = self.options.size {
                attributes =
                    attributes.with_inner_size(winit::dpi::LogicalSize::new(width, height));
            }
            let window = Arc::new(
                event_loop
                    .create_window(attributes)
                    .map_err(|error| format!("iOS window creation failed: {error}"))?,
            );
            let physical = window.inner_size();
            let scale = window.scale_factor() as f32;
            stage(
                "window",
                &format!(
                    "created pid={} physical={}x{} scale={scale}",
                    std::process::id(),
                    physical.width,
                    physical.height
                ),
            );
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
            let surface = instance
                .create_surface(window.clone())
                .map_err(|error| format!("iOS Metal surface: {error}"))?;
            let host = pollster::block_on(wgpu_context::DeviceHandle::new_from_compatible_surface(
                instance,
                Some(&surface),
                None,
                None,
            ))
            .map_err(|error| format!("wgpu iOS device: {error}"))?;
            stage(
                "adapter",
                &format!(
                    "name=\"{}\" backend={}",
                    host.adapter.get_info().name,
                    host.adapter.get_info().backend
                ),
            );
            let caps = surface.get_capabilities(&host.adapter);
            let format = [
                wgpu::TextureFormat::Bgra8UnormSrgb,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureFormat::Rgba8Unorm,
            ]
            .into_iter()
            .find(|format| caps.formats.contains(format))
            .ok_or("no supported iOS surface format")?;
            let mut config = surface
                .get_default_config(
                    &host.adapter,
                    physical.width.max(1),
                    physical.height.max(1),
                )
                .ok_or("iOS surface configuration")?;
            config.format = format;
            config.present_mode = wgpu::PresentMode::AutoVsync;
            config.usage |= wgpu::TextureUsages::COPY_SRC;
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
            self.generation = if state.state == SurfaceState::Suspended {
                self.engine
                    .resume(
                        OWNER,
                        self.generation,
                        physical.width.max(1),
                        physical.height.max(1),
                        scale,
                    )
                    .map_err(|error| format!("native resume rejected: {error:?}"))?
            } else {
                self.generation
            };
            self.device = Some(host.device);
            self.queue = Some(host.queue);
            self.surface = Some(surface);
            self.config = Some(config);
            self.gpu = Some(gpu);
            self.window = Some(window.clone());
            self.suspended = false;
            self.apply_viewport()?;
            let _ = self.engine.focus(
                OWNER,
                self.scene.version(),
                self.generation,
                BUTTON,
                &mut |_| {},
            );
            window.request_redraw();
            Ok(())
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() || self.closed {
                return;
            }
            let action = self.lifecycle.transition(crate::model::LifecycleEvent::DidBecomeActive);
            stage("lifecycle", &format!("{action:?} state={:?}", self.lifecycle.state()));
            if matches!(action, LifecycleAction::None) && self.lifecycle.state() != crate::model::LifecycleState::Active {
                return;
            }
            if let Err(error) = self.resume(event_loop) {
                self.error = Some(error.clone());
                stage("error", &error);
                event_loop.exit();
            }
        }

        fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
            let action = self.lifecycle.transition(crate::model::LifecycleEvent::WillResignActive);
            stage("lifecycle", &format!("{action:?} state={:?}", self.lifecycle.state()));
            if action != LifecycleAction::Suspend || self.window.is_none() {
                return;
            }
            match self.engine.suspend(
                OWNER,
                self.scene.version(),
                self.generation,
                &mut |_| {},
            ) {
                Ok(generation) => {
                    self.generation = generation;
                    self.suspended = true;
                    stage("suspend", &format!("generation={generation}"));
                }
                Err(error) => {
                    self.error = Some(format!("native suspend rejected: {error:?}"));
                    stage("error", &format!("native suspend rejected: {error:?}"));
                }
            }
            self.teardown_surface();
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
                        self.error = Some(error.clone());
                        stage("error", &error);
                        event_loop.exit();
                        return;
                    }
                    if self.options.frames.is_some_and(|frames| self.frames >= frames) {
                        self.shutdown();
                        event_loop.exit();
                    }
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    if let Err(error) = self
                        .configure_surface()
                        .and_then(|()| self.apply_viewport())
                    {
                        self.error = Some(error.clone());
                        stage("error", &error);
                        event_loop.exit();
                        return;
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                other => {
                    let inputs = self.input.translate(&other);
                    if inputs.is_empty() {
                        return;
                    }
                    if let Err(error) = self.dispatch_inputs(inputs) {
                        self.error = Some(error.clone());
                        stage("error", &error);
                        event_loop.exit();
                        return;
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            let waiting = self
            .options
            .frames
            .is_some_and(|frames| self.frames < frames)
            || (self.options.tap.is_some() || self.options.tap_center) && !self.tapped;
            if self.window.is_some() && waiting && self.frames == 0 {
                // Retry while the drawable is not ready yet; bounded by the host watchdog.
                // drawable 尚未就绪时重试；由宿主看门狗兜底。
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + std::time::Duration::from_millis(16),
                ));
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }

        fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {}

        fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
            if !self.suspended {
                self.shutdown();
            }
        }

        fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
            stage("timeout", "host watchdog expired");
            self.error = Some("host watchdog expired before retirement".into());
            self.shutdown();
            event_loop.exit();
        }
    }

    /// Read the presented drawable back through a wgpu staging buffer and write a PPM.
    /// 经 wgpu 暂存缓冲读回已呈现 drawable 并写出 PPM。
    fn read_frame(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        path: &str,
    ) -> Result<FrameRecord, String> {
        let unpadded = width.checked_mul(4).ok_or("readback row overflow")?;
        let padded = unpadded.div_ceil(256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ios-readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ios-readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        NonZeroU32::new(padded)
                            .ok_or_else(|| "readback stride".to_string())?
                            .get(),
                    ),
                    rows_per_image: Some(
                        NonZeroU32::new(height)
                            .ok_or_else(|| "readback height".to_string())?
                            .get(),
                    ),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| format!("iOS readback poll: {error}"))?;
        receiver
            .recv()
            .map_err(|error| format!("iOS readback channel: {error}"))?
            .map_err(|error| format!("iOS readback map: {error}"))?;

        let data = slice
            .get_mapped_range()
            .map_err(|error| format!("iOS readback view: {error}"))?;
        let bgra = matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let encode = |color: Color| {
            (
                srgb_encode(color.r),
                srgb_encode(color.g),
                srgb_encode(color.b),
            )
        };
        let (background, active, pressed) =
            (encode(BACKGROUND), encode(ACTIVE), encode(PRESSED));
        let mut ppm = Vec::with_capacity((width * height * 3 + 32) as usize);
        ppm.extend_from_slice(format!("P6\n{width} {height}\n255\n").as_bytes());
        let mut counts = [0usize; 4];
        for row in 0..height {
            let start = (row * padded) as usize;
            for column in 0..width {
                let offset = start + (column * 4) as usize;
                let (r, g, b) = if bgra {
                    (data[offset + 2], data[offset + 1], data[offset])
                } else {
                    (data[offset], data[offset + 1], data[offset + 2])
                };
                ppm.extend_from_slice(&[r, g, b]);
                let bucket = if (r, g, b) == background {
                    0
                } else if (r, g, b) == active {
                    1
                } else if (r, g, b) == pressed {
                    2
                } else {
                    3
                };
                counts[bucket] += 1;
            }
        }
        drop(data);
        buffer.unmap();
        std::fs::write(path, &ppm).map_err(|error| format!("readback write {path}: {error}"))?;
        Ok(FrameRecord {
            path: path.to_string(),
            bytes: ppm.len(),
            background: counts[0],
            active: counts[1],
            pressed: counts[2],
            other: counts[3],
            sha256: sha256_hex(&ppm),
        })
    }

    /// Encode one 8-bit linear channel to the 8-bit sRGB value an sRGB drawable stores.
    /// 把一个 8 位线性通道编码为 sRGB drawable 实际存储的 8 位值。
    fn srgb_encode(value: u8) -> u8 {
        let linear = f64::from(value) / 255.0;
        let encoded = if linear <= 0.003_130_8 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (encoded * 255.0).round().clamp(0.0, 255.0) as u8
    }

    /// Dependency-free SHA-256 for evidence digests.
    /// 无依赖的 SHA-256，用于证据摘要。
    fn sha256_hex(bytes: &[u8]) -> String {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let mut message = bytes.to_vec();
        let bit_len = (bytes.len() as u64).wrapping_mul(8);
        message.push(0x80);
        while message.len() % 64 != 56 {
            message.push(0);
        }
        message.extend_from_slice(&bit_len.to_be_bytes());
        for block in message.chunks_exact(64) {
            let mut w = [0u32; 64];
            for (index, word) in block.chunks_exact(4).enumerate() {
                w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
            }
            for index in 16..64 {
                let s0 = w[index - 15].rotate_right(7)
                    ^ w[index - 15].rotate_right(18)
                    ^ (w[index - 15] >> 3);
                let s1 = w[index - 2].rotate_right(17)
                    ^ w[index - 2].rotate_right(19)
                    ^ (w[index - 2] >> 10);
                w[index] = w[index - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[index - 7])
                    .wrapping_add(s1);
            }
            let mut v = h;
            for index in 0..64 {
                let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
                let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
                let temp1 = v[7]
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[index])
                    .wrapping_add(w[index]);
                let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
                let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
                let temp2 = s0.wrapping_add(maj);
                v[7] = v[6];
                v[6] = v[5];
                v[5] = v[4];
                v[4] = v[3].wrapping_add(temp1);
                v[3] = v[2];
                v[2] = v[1];
                v[1] = v[0];
                v[0] = temp1.wrapping_add(temp2);
            }
            for index in 0..8 {
                h[index] = h[index].wrapping_add(v[index]);
            }
        }
        h.iter().map(|word| format!("{word:08x}")).collect()
    }

    pub fn main() {
        let (options, font) = match parse_args() {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        };
        let event_loop = match EventLoop::<()>::with_user_event().build() {
            Ok(event_loop) => event_loop,
            Err(error) => {
                eprintln!("iOS event loop failed: {error}");
                std::process::exit(2);
            }
        };
        let cjk = match &options.cjk_font {
            Some(path) => match std::fs::read(path) {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    eprintln!("cjk font {path}: {error}");
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let mut app = match App::new(options, &font, cjk.as_deref()) {
            Ok(app) => app,
            Err(error) => {
                eprintln!("iOS host init failed: {error}");
                std::process::exit(2);
            }
        };
        let timeout_ms = app.options.timeout_ms;
        if let Some(timeout) = timeout_ms {
            let proxy = event_loop.create_proxy();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(timeout));
                let _ = proxy.send_event(());
            });
        }
        if let Err(error) = event_loop.run_app(&mut app) {
            eprintln!("iOS event loop failed: {error}");
            std::process::exit(2);
        }
        println!("{}", app.summary());
        for record in &app.records {
            println!(
                "readback path={} bytes={} background={} active={} pressed={} other={} sha256={}",
                record.path,
                record.bytes,
                record.background,
                record.active,
                record.pressed,
                record.other,
                record.sha256
            );
        }
        if let Some(error) = &app.error {
            eprintln!("iOS host error: {error}");
            std::process::exit(1);
        }
        if !app.closed {
            eprintln!("iOS host did not retire its native view");
            std::process::exit(1);
        }
    }
}

#[cfg(any(target_os = "ios", feature = "host-on-macos"))]
fn main() {
    host::main();
}
