//! macOS reference host for the Jasper UI native engine.
//! Jasper UI 原生引擎的 macOS 参考宿主。
//!
//! The host owns the Cocoa window, the `CAMetalLayer` surface, the wgpu Metal device/queue and
//! the AccessKit Cocoa adapter. The engine stays host-neutral and receives only owned scene
//! values, normalized pointer/keyboard events and surface generations.
//!
//! 宿主持有 Cocoa 窗口、`CAMetalLayer` Surface、wgpu Metal 设备/队列与 AccessKit Cocoa 适配器；
//! 引擎保持宿主中立，只接收自有场景值、规范化指针/键盘事件与 Surface 代数。
//!
//! Run:
//! `jasper-ui FONT [--once] [--frames N] [--readback out.ppm] [--immediate] [--tap] [--timeout-ms N]`
//!
//! This is a reference platform adapter, not a macOS application framework.

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
use jasper_ui_engine::style::{Align, Color, FlexDirection, FontStyle, Justify, Length, Style};
use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::{Window, WindowId};

/// User events: AccessKit bridge traffic plus a host-owned watchdog.
/// 用户事件：AccessKit 桥流量加上宿主持有的看门狗。
enum AppEvent {
    Accessibility(accesskit_winit::Event),
    Timeout,
    Exit,
}

impl From<accesskit_winit::Event> for AppEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::Accessibility(event)
    }
}

const OWNER: u64 = 0x4a53_5045_522d_4d41; // "JASPER-MA"
const BUTTON: NodeId = NodeId(2);
const BUDGET: usize = 64 * 1024 * 1024;
/// Host-injected ordered font stack: Latin first, CJK fallback second, matching the
/// Android and Windows reference hosts.
/// 宿主注入的有序字体栈：拉丁在前、CJK 回退在后，与 Android / Windows 参考宿主一致。
const LATIN: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");
const CJK: &[u8] = include_bytes!("../assets/DroidSansFallbackFull.ttf");

const BACKGROUND: Color = Color::rgb(18, 20, 26);
const ACTIVE: Color = Color::rgb(40, 130, 90);
const PRESSED: Color = Color::rgb(190, 92, 52);

/// Machine-readable progress line. Automation greps these `stage=` lines.
/// 机器可读的进度行；自动化脚本据此匹配 `stage=`。
fn stage(name: &str, detail: &str) {
    eprintln!("stage={name} {detail}");
    log::info!("stage={name} {detail}");
}

struct Options {
    once: bool,
    frames: Option<u64>,
    readback: Option<String>,
    immediate: bool,
    tap: bool,
    timeout_ms: Option<u64>,
    /// Optional CJK font file the host injects after the Latin font.
    /// 宿主在拉丁字体之后注入的可选中日韩字体文件。
    exit_after_ms: Option<u64>,
    /// Keep rendering until the frame target is reached instead of waiting for events.
    /// 持续渲染直到达到帧目标，而不是等待事件。
    spin: bool,
    cjk_font: Option<String>,
    /// Add the Chinese line without a CJK font, to measure the fallback difference.
    /// 只加中文行、不注入中日韩字体，用于测量回退差异。
    cjk_tofu: bool,
    /// Which single screen to render.
    /// 渲染哪一个界面。
    scene: SceneKind,
    /// Logical window size.
    /// 逻辑窗口尺寸。
    size: (f32, f32),
}

/// Selectable single-screen scenes.
/// 可选的单屏界面。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SceneKind {
    /// Reference acceptance screen (title, Chinese line, button).
    /// 参考验收界面（标题、中文行、按钮）。
    Reference,
    /// High-fidelity macOS-style settings mockup.
    /// 高保真 macOS 风格设置界面原型。
    Mockup,
}

fn parse_args() -> Result<(Options, Vec<u8>), String> {
    let mut font_path = None;
    let mut options = Options {
        once: false,
        frames: None,
        readback: None,
        immediate: false,
        tap: false,
        timeout_ms: None,
        exit_after_ms: None,
        spin: false,
        cjk_font: None,
        cjk_tofu: false,
        scene: SceneKind::Reference,
        size: (960.0, 640.0),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--once" => options.once = true,
            "--tap" => options.tap = true,
            "--spin" => options.spin = true,
            "--cjk-tofu" => options.cjk_tofu = true,
            "--scene" => {
                let value = args.next().ok_or("--scene needs reference|mockup")?;
                options.scene = match value.as_str() {
                    "reference" => SceneKind::Reference,
                    "mockup" => SceneKind::Mockup,
                    other => return Err(format!("unknown scene: {other}")),
                };
            }
            "--size" => {
                let value = args.next().ok_or("--size needs WIDTHxHEIGHT")?;
                let (width, height) = value
                    .split_once('x')
                    .ok_or("--size needs WIDTHxHEIGHT")?;
                options.size = (
                    width.parse().map_err(|_| "invalid --size width")?,
                    height.parse().map_err(|_| "invalid --size height")?,
                );
            }
            "--cjk-font" => {
                options.cjk_font = Some(args.next().ok_or("--cjk-font needs a path")?);
            }
            "--immediate" => options.immediate = true,
            "--frames" => {
                let value = args.next().ok_or("--frames needs a count")?;
                options.frames = Some(value.parse().map_err(|_| "invalid --frames count")?);
            }
            "--readback" => {
                options.readback = Some(args.next().ok_or("--readback needs a path")?);
            }
            "--timeout-ms" => {
                let value = args.next().ok_or("--timeout-ms needs a number")?;
                options.timeout_ms = Some(value.parse().map_err(|_| "invalid --timeout-ms")?);
            }
            "--exit-after-ms" => {
                let value = args.next().ok_or("--exit-after-ms needs a number")?;
                options.exit_after_ms = Some(
                    value
                        .parse()
                        .map_err(|_| "invalid --exit-after-ms")?,
                );
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
    let font_path = font_path.ok_or("usage: jasper-ui FONT [--once] [options]")?;
    let font = std::fs::read(&font_path).map_err(|error| format!("font {font_path}: {error}"))?;
    Ok((options, font))
}


// ----- High-fidelity macOS-style mockup -------------------------------------
// ----- 高保真 macOS 风格界面原型 ---------------------------------------------

const M_CONTENT: Color = Color::rgb(245, 245, 247);
const M_SIDEBAR: Color = Color::rgb(233, 233, 236);
const M_CARD: Color = Color::rgb(255, 255, 255);
const M_BORDER: Color = Color::rgb(227, 227, 230);
const M_TEXT: Color = Color::rgb(29, 29, 31);
const M_SECONDARY: Color = Color::rgb(110, 110, 115);
const M_ACCENT: Color = Color::rgb(10, 132, 255);
const M_GREEN: Color = Color::rgb(52, 199, 89);
const M_DIVIDER: Color = Color::rgb(237, 237, 240);
const M_SWITCH_OFF: Color = Color::rgb(233, 233, 235);

fn edge(top: f32, right: f32, bottom: f32, left: f32) -> Edges {
    Edges {
        top,
        right,
        bottom,
        left,
    }
}

fn m_text(id: u64, value: &str, size: f32, color: Color, weight: u16) -> Node {
    Node::text(NodeId(id), value).with_style(
        Style::default()
            .width(Length::Auto)
            .height(Length::Auto)
            .text(size, color)
            .font(weight, FontStyle::Normal),
    )
}

fn m_traffic_light(id: u64, color: Color) -> Node {
    Node::container(NodeId(id)).with_style(
        Style::default()
            .size(Length::Px(12.0), Length::Px(12.0))
            .background(color)
            .rounded(6.0),
    )
}

/// One sidebar row: colored icon tile plus label.
fn m_sidebar_item(id: u64, icon: Color, label: &str, selected: bool) -> Node {
    let foreground = if selected { Color::WHITE } else { M_TEXT };
    Node::container(NodeId(id))
        .with_style(
            Style::default()
                .width(Length::Percent(100.0))
                .height(Length::Px(32.0))
                .direction(FlexDirection::Row)
                .align_items(Align::Center)
                .gap(10.0)
                .padding(edge(0.0, 8.0, 0.0, 8.0))
                .background(if selected { M_ACCENT } else { Color::TRANSPARENT })
                .rounded(8.0),
        )
        .child(Node::container(NodeId(id * 100 + 1)).with_style(
            Style::default()
                .size(Length::Px(18.0), Length::Px(18.0))
                .background(if selected { Color::WHITE.with_alpha(235) } else { icon })
                .rounded(5.0),
        ))
        .child(m_text(id * 100 + 2, label, 13.0, foreground, 500))
}

fn m_divider(id: u64) -> Node {
    Node::container(NodeId(id)).with_style(
        Style::default()
            .width(Length::Percent(100.0))
            .height(Length::Px(1.0))
            .background(M_DIVIDER),
    )
}

/// A settings row inside a card: title on the left, detail on the right.
fn m_row(id: u64, title: &str, detail: &str, accent: bool) -> Node {
    Node::container(NodeId(id))
        .with_style(
            Style::default()
                .width(Length::Percent(100.0))
                .height(Length::Px(48.0))
                .direction(FlexDirection::Row)
                .align_items(Align::Center)
                .justify_content(Justify::SpaceBetween)
                .padding(edge(0.0, 16.0, 0.0, 16.0)),
        )
        .child(m_text(id * 100 + 1, title, 13.0, M_TEXT, 400))
        .child(m_text(
            id * 100 + 2,
            detail,
            12.0,
            if accent { M_ACCENT } else { M_SECONDARY },
            400,
        ))
}

/// A card container with the standard Apple surface.
fn m_card(id: u64) -> Node {
    Node::container(NodeId(id)).with_style(
        Style::default()
            .width(Length::Percent(100.0))
            .direction(FlexDirection::Column)
            .background(M_CARD)
            .border(1.0, M_BORDER)
            .rounded(12.0),
    )
}

fn mockup_scene() -> Scene {
    let sidebar = Node::container(NodeId(2))
        .with_style(
            Style::default()
                .width(Length::Px(262.0))
                .height(Length::Percent(100.0))
                .direction(FlexDirection::Column)
                .padding(edge(16.0, 14.0, 16.0, 14.0))
                .gap(4.0)
                .background(M_SIDEBAR)
                .border(1.0, Color::rgb(222, 222, 226)),
        )
        .child(
            Node::container(NodeId(3))
                .with_style(
                    Style::default()
                        .width(Length::Percent(100.0))
                        .direction(FlexDirection::Row)
                        .gap(8.0)
                        .margin(edge(0.0, 0.0, 14.0, 0.0)),
                )
                .child(m_traffic_light(4, Color::rgb(255, 95, 87)))
                .child(m_traffic_light(5, Color::rgb(254, 188, 46)))
                .child(m_traffic_light(6, Color::rgb(40, 200, 64))),
        )
        .child(
            Node::container(NodeId(7))
                .with_style(
                    Style::default()
                        .width(Length::Percent(100.0))
                        .height(Length::Px(30.0))
                        .direction(FlexDirection::Row)
                        .align_items(Align::Center)
                        .justify_content(Justify::SpaceBetween),
                )
                .child(m_text(8, "系统设置", 15.0, M_TEXT, 600))
                .child(m_text(9, "⌘,", 12.0, M_SECONDARY, 400)),
        )
        .child(
            Node::container(NodeId(10))
                .with_style(
                    Style::default()
                        .width(Length::Percent(100.0))
                        .height(Length::Px(30.0))
                        .direction(FlexDirection::Row)
                        .align_items(Align::Center)
                        .padding(edge(0.0, 8.0, 0.0, 8.0))
                        .margin(edge(6.0, 0.0, 10.0, 0.0))
                        .background(M_CARD)
                        .border(1.0, M_BORDER)
                        .rounded(8.0),
                )
                .child(m_text(11, "搜索", 12.0, M_SECONDARY, 400)),
        )
        .child(m_text(12, "控制中心", 11.0, M_SECONDARY, 600))
        .child(m_sidebar_item(13, Color::rgb(10, 132, 255), "Wi-Fi", true))
        .child(m_sidebar_item(14, Color::rgb(0, 122, 255), "蓝牙", false))
        .child(m_sidebar_item(15, Color::rgb(142, 142, 147), "网络", false))
        .child(m_sidebar_item(16, Color::rgb(255, 149, 0), "通知", false))
        .child(m_sidebar_item(17, Color::rgb(255, 45, 85), "声音", false))
        .child(m_text(18, "其他", 11.0, M_SECONDARY, 600))
        .child(m_sidebar_item(19, Color::rgb(88, 86, 214), "外观", false))
        .child(m_sidebar_item(20, Color::rgb(48, 176, 199), "控制中心", false))
        .child(m_sidebar_item(21, Color::rgb(162, 132, 94), "桌面与程序坞", false));

    let content = Node::container(NodeId(40))
        .with_style(
            Style::default()
                .flex_grow(1.0)
                .height(Length::Percent(100.0))
                .direction(FlexDirection::Column)
                .padding(edge(26.0, 30.0, 26.0, 30.0))
                .gap(18.0)
                .background(M_CONTENT),
        )
        .child(
            Node::container(NodeId(41))
                .with_style(
                    Style::default()
                        .width(Length::Percent(100.0))
                        .direction(FlexDirection::Column)
                        .gap(4.0),
                )
                .child(m_text(42, "Wi-Fi", 26.0, M_TEXT, 700))
                .child(m_text(43, "已连接「Jasper 5G」", 12.0, M_SECONDARY, 400)),
        )
        .child(
            m_card(50)
                .child(
                    Node::container(NodeId(51))
                        .with_style(
                            Style::default()
                                .width(Length::Percent(100.0))
                                .height(Length::Px(58.0))
                                .direction(FlexDirection::Row)
                                .align_items(Align::Center)
                                .justify_content(Justify::SpaceBetween)
                                .padding(edge(0.0, 16.0, 0.0, 16.0)),
                        )
                        .child(
                            Node::container(NodeId(52))
                                .with_style(
                                    Style::default()
                                        .direction(FlexDirection::Column)
                                        .gap(2.0),
                                )
                                .child(m_text(53, "Wi-Fi", 13.0, M_TEXT, 500))
                                .child(m_text(54, "已打开", 12.0, M_SECONDARY, 400)),
                        )
                        .child(
                            Node::container(NodeId(55))
                                .with_style(
                                    Style::default()
                                        .size(Length::Px(44.0), Length::Px(26.0))
                                        .direction(FlexDirection::Row)
                                        .justify_content(Justify::End)
                                        .align_items(Align::Center)
                                        .padding(edge(0.0, 2.0, 0.0, 2.0))
                                        .background(M_GREEN)
                                        .rounded(13.0),
                                )
                                .child(Node::container(NodeId(56)).with_style(
                                    Style::default()
                                        .size(Length::Px(22.0), Length::Px(22.0))
                                        .background(Color::WHITE)
                                        .rounded(11.0),
                                )),
                        ),
                )
                .child(m_divider(57))
                .child(m_row(58, "已知网络", "›", false)),
        )
        .child(
            m_card(60)
                .child(m_row(61, "Jasper 5G", "已连接", true))
                .child(m_divider(62))
                .child(m_row(63, "Jasper-Guest", "未连接", false))
                .child(m_divider(64))
                .child(m_row(65, "办公室", "自动加入", false)),
        )
        .child(
            m_card(70).child(
                Node::container(NodeId(71))
                    .with_style(
                        Style::default()
                            .width(Length::Percent(100.0))
                            .direction(FlexDirection::Column)
                            .gap(12.0)
                            .padding(edge(14.0, 16.0, 14.0, 16.0)),
                    )
                    .child(m_text(72, "自动加入网络", 12.0, M_SECONDARY, 400))
                    .child(
                        Node::container(NodeId(73))
                            .with_style(
                                Style::default()
                                    .width(Length::Percent(100.0))
                                    .height(Length::Px(32.0))
                                    .direction(FlexDirection::Row)
                                    .gap(2.0)
                                    .padding(edge(3.0, 3.0, 3.0, 3.0))
                                    .background(Color::rgb(233, 233, 235))
                                    .rounded(9.0),
                            )
                            .child(
                                Node::container(NodeId(74))
                                    .with_style(
                                        Style::default()
                                            .flex_grow(1.0)
                                            .height(Length::Px(26.0))
                                            .align_items(Align::Center)
                                            .justify_content(Justify::Center)
                                            .background(M_CARD)
                                            .rounded(7.0),
                                    )
                                    .child(m_text(75, "自动", 12.0, M_TEXT, 500)),
                            )
                            .child(
                                Node::container(NodeId(76))
                                    .with_style(
                                        Style::default()
                                            .flex_grow(1.0)
                                            .height(Length::Px(26.0))
                                            .align_items(Align::Center)
                                            .justify_content(Justify::Center),
                                    )
                                    .child(m_text(77, "手动", 12.0, M_SECONDARY, 400)),
                            )
                            .child(
                                Node::container(NodeId(78))
                                    .with_style(
                                        Style::default()
                                            .flex_grow(1.0)
                                            .height(Length::Px(26.0))
                                            .align_items(Align::Center)
                                            .justify_content(Justify::Center),
                                    )
                                    .child(m_text(79, "关闭", 12.0, M_SECONDARY, 400)),
                            ),
                    )
                    .child(
                        Node::container(NodeId(80))
                            .with_style(
                                Style::default()
                                    .width(Length::Percent(100.0))
                                    .height(Length::Px(18.0))
                                    .direction(FlexDirection::Row)
                                    .align_items(Align::Center)
                                    .gap(10.0),
                            )
                            .child(m_text(81, "亮度", 12.0, M_SECONDARY, 400))
                            .child(
                                Node::container(NodeId(82))
                                    .with_style(
                                        Style::default()
                                            .flex_grow(1.0)
                                            .height(Length::Px(4.0))
                                            .background(Color::rgb(217, 217, 222))
                                            .rounded(2.0),
                                    )
                                    .child(
                                        Node::container(NodeId(83)).with_style(
                                            Style::default()
                                                .width(Length::Percent(62.0))
                                                .height(Length::Px(4.0))
                                                .background(M_ACCENT)
                                                .rounded(2.0),
                                        ),
                                    ),
                            )
                            .child(
                                Node::container(NodeId(84)).with_style(
                                    Style::default()
                                        .size(Length::Px(16.0), Length::Px(16.0))
                                        .background(M_CARD)
                                        .border(1.0, Color::rgb(200, 200, 205))
                                        .rounded(8.0),
                                ),
                            ),
                    ),
            ),
        );

    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    // The mockup window is fixed; explicit pixels keep the root from shrinking
                    // to its content inside Blitz's centering wrapper.
                    // 原型窗口尺寸固定；显式像素避免根节点在 Blitz 的居中包装里收缩成内容尺寸。
                    .width(Length::Px(1180.0))
                    .height(Length::Px(760.0))
                    .direction(FlexDirection::Row)
                    .background(M_CONTENT),
            )
            .child(sidebar)
            .child(content),
    )
}

fn scene(chinese: bool) -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .width(Length::Percent(100.0))
                    .height(Length::Percent(100.0))
                    .padding(Edges::all(32.0))
                    .gap(16.0)
                    .direction(FlexDirection::Column)
                    .justify_content(Justify::Center)
                    .align_items(Align::Center)
                    .background(BACKGROUND),
            )
            .child(
                Node::text(NodeId(3), "Jasper UI · macOS")
                    .with_style(
                        Style::default()
                            .background(BACKGROUND)
                            .size(Length::Percent(80.0), Length::Px(44.0))
                            .text(28.0, Color::rgb(230, 234, 240)),
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
                            .width(Length::Percent(70.0))
                            .height(Length::Px(120.0))
                            .background(ACTIVE)
                            .text(32.0, Color::rgb(245, 247, 250)),
                    ),
            ),
    )
}

fn root_style(width: f32, height: f32) -> Style {
    Style::default()
        .width(Length::Px(width))
        .height(Length::Px(height))
        .padding(Edges::all(32.0))
        .gap(16.0)
        .direction(FlexDirection::Column)
        .justify_content(Justify::Center)
        .align_items(Align::Center)
        .background(BACKGROUND)
}

fn logical_extent(window: &Window) -> (u32, u32, f32) {
    let size = window.inner_size();
    let scale = window.scale_factor() as f32;
    (size.width.max(1), size.height.max(1), scale)
}

/// One presented frame plus its measured pixel statistics.
/// 一帧已呈现画面及其实测像素统计。
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
    activations: u64,
    frames: u64,
    suspended: bool,
    closed: bool,
    started: Instant,
    queued_tap: bool,
    tapped: bool,
    records: Vec<FrameRecord>,
    viewport: Option<(u32, u32, f32)>,
    /// Consecutive redraw attempts that found no drawable yet.
    /// 连续未取得 drawable 的重绘尝试次数。
    pending_surface: u32,
    error: Option<String>,
    accessibility: Option<accesskit_winit::Adapter>,
    event_proxy: EventLoopProxy<AppEvent>,
}

impl App {
    fn new(
        options: Options,
        latin: &[u8],
        cjk: Option<&[u8]>,
        event_proxy: EventLoopProxy<AppEvent>,
    ) -> Result<Self, String> {
        let chinese = !options.cjk_tofu;
        let source = match options.scene {
            SceneKind::Reference => scene(chinese),
            SceneKind::Mockup => mockup_scene(),
        };
        // `--cjk-tofu` deliberately drops the fallback face to measure the difference.
        // `--cjk-tofu` 故意去掉回退字面，用于测量差异。
        let stack: Vec<&[u8]> = if options.cjk_tofu {
            vec![latin]
        } else {
            vec![latin, cjk.unwrap_or(CJK)]
        };
        let (width, height) = (options.size.0.round() as u32, options.size.1.round() as u32);
        let engine = HostRenderer::with_fonts(OWNER, &source, &stack, width, height)
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
            activations: 0,
            frames: 0,
            suspended: false,
            closed: false,
            started: Instant::now(),
            queued_tap: false,
            tapped: false,
            records: Vec::new(),
            viewport: None,
            pending_surface: 0,
            error: None,
            accessibility: None,
            event_proxy,
        })
    }

    fn configure_surface(&mut self) -> Result<(), String> {
        let window = self.window.clone().ok_or("missing macOS window")?;
        let (width, height, _) = logical_extent(&window);
        let device = self.device.as_ref().ok_or("missing wgpu device")?;
        let config = self.config.as_mut().ok_or("missing surface configuration")?;
        if config.width == width && config.height == height {
            return Ok(());
        }
        config.width = width;
        config.height = height;
        self.surface
            .as_ref()
            .ok_or("missing macOS surface")?
            .configure(device, config);
        Ok(())
    }

    /// Reconfigure the engine viewport to the current physical surface extent.
    /// 按当前物理 Surface 尺寸重配引擎 viewport。
    fn reconfigure_engine(&mut self) -> Result<(), String> {
        let window = self.window.clone().ok_or("missing macOS window")?;
        let (width, height, scale) = logical_extent(&window);
        if !scale.is_finite() || scale <= 0.0 {
            return Err("macOS DPI scale is invalid".into());
        }
        if self.viewport == Some((width, height, scale)) {
            return Ok(());
        }
        self.viewport = Some((width, height, scale));
        self.generation = self
            .engine
            .reconfigure(OWNER, self.generation, width, height, scale)
            .map_err(|error| format!("native viewport rejected: {error:?}"))?;
        let old = self.scene.version();
        // The reference screen re-centres its column on every resize; the mockup keeps its own
        // fixed-pixel root, so patching it would overwrite the design.
        // 参考界面在每次尺寸变化时重排居中列；原型界面保持自己的固定像素根，打补丁会覆盖设计。
        let patches: Vec<ScenePatch> = if self.options.scene == SceneKind::Reference {
            vec![ScenePatch::UpdateStyle {
                id: NodeId(1),
                style: root_style(width as f32 / scale, height as f32 / scale),
            }]
        } else {
            Vec::new()
        };
        if !patches.is_empty() {
            self.scene
                .apply_batch_at(old, &patches)
                .map_err(|error| format!("macOS root resize rejected: {error:?}"))?;
            if !self.engine.apply_with_focus_events(
                OWNER,
                old,
                self.scene.version(),
                self.generation,
                &patches,
                &mut |_| {},
            ) {
                return Err("native macOS root resize rejected".into());
            }
        }
        stage(
            "viewport",
            &format!("width={width} height={height} scale={scale} generation={}", self.generation),
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

    /// Route one engine-level pointer click at a logical position.
    /// 在逻辑坐标处路由一次引擎级指针点击。
    fn click(&mut self, x: f32, y: f32) -> Result<bool, String> {
        let mut activated = false;
        for (action, pressed) in [(PointerAction::Down, true), (PointerAction::Up, false)] {
            let mut hit = false;
            if !self.engine.pointer(
                OWNER,
                self.scene.version(),
                self.generation,
                action,
                x,
                y,
                pressed,
                &mut |event| {
                    hit |= event.kind == jasper_ui_engine::native::SceneEventKind::Activate;
                    EventReply::default()
                },
                &mut |_| {},
            ) {
                return Err("native macOS pointer rejected".into());
            }
            activated |= hit;
        }
        if activated {
            self.activate()?;
        }
        Ok(activated)
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
                .width(Length::Percent(70.0))
                .height(Length::Px(120.0))
                .background(background)
                .text(32.0, Color::rgb(245, 247, 250)),
        };
        self.scene
            .apply_batch_at(old, std::slice::from_ref(&patch))
            .map_err(|error| format!("macOS source update rejected: {error:?}"))?;
        if !self.engine.apply_with_focus_events(
            OWNER,
            old,
            self.scene.version(),
            self.generation,
            std::slice::from_ref(&patch),
            &mut |_| {},
        ) {
            return Err("native macOS style update rejected".into());
        }
        stage(
            "activate",
            &format!("count={} version={}", self.activations, self.scene.version()),
        );
        Ok(())
    }

    fn dispatch_inputs(&mut self, inputs: Vec<InputEvent>) -> Result<(), String> {
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        for input in inputs {
            match input {
                InputEvent::Pointer(pointer) => {
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
                            activated |= event.kind
                                == jasper_ui_engine::native::SceneEventKind::Activate;
                            EventReply::default()
                        },
                        &mut |_| {},
                    ) {
                        return Err("native macOS pointer rejected".into());
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
                        return Err("native macOS deactivation rejected".into());
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
        let window = self.window.clone().ok_or("missing macOS window")?;
        let (width, height, scale) = logical_extent(&window);
        let config = self.config.as_ref().ok_or("missing surface configuration")?;
        let format = config.format;
        let acquired = self
            .surface
            .as_ref()
            .ok_or("missing macOS surface")?
            .get_current_texture();
        if let wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded =
            &acquired
        {
            self.pending_surface = self.pending_surface.saturating_add(1);
            stage(
                "surface",
                &format!("not available yet attempt={}", self.pending_surface),
            );
        }
        let frame = match acquired {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure_surface()?;
                self.reconfigure_engine()?;
                window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("macOS surface validation failed".into())
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
            .map_err(|error| format!("macOS GPU frame rejected: {error:?}"))?;
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
            return Err("macOS GPU present acknowledgement rejected".into());
        }
        self.frames += 1;
        self.pending_surface = 0;
        if let Some(record) = readback {
            stage(
                "frame",
                &format!(
                    "index={} {}x{} scale={scale} format={format:?} background={} active={} pressed={} other={} bytes={} sha256={}",
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
        } else {
            stage(
                "frame",
                &format!(
                    "index={} {}x{} scale={scale} format={format:?}",
                    self.frames, width, height
                ),
            );
        }
        // One host-injected activation after the first presented frame proves routing, the
        // versioned visual patch and a second frame; it is not an OS-level click.
        // 首帧呈现后注入一次宿主级激活，验证路由、版本化视觉补丁与第二帧；它不是 OS 级点击。
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
        if self.options.tap && !self.queued_tap && self.frames >= 1 {
            self.queued_tap = true;
            if let Some((x, y)) = self.button_center() {
                self.tapped = self.click(x, y)?;
                stage("tap", &format!("x={x} y={y} activated={}", self.tapped));
                window.request_redraw();
            } else {
                stage("tap", "button bounds unavailable");
            }
        }
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

    fn suspend(&mut self) -> Result<(), String> {
        self.generation = self
            .engine
            .suspend(OWNER, self.scene.version(), self.generation, &mut |_| {})
            .map_err(|error| format!("native suspend rejected: {error:?}"))?;
        self.surface = None;
        self.config = None;
        self.device = None;
        self.queue = None;
        self.gpu = None;
        self.window = None;
        self.suspended = true;
        stage("suspend", &format!("generation={}", self.generation));
        Ok(())
    }

    fn summary(&self) -> String {
        let (width, height, scale) = self
            .window
            .as_ref()
            .map_or((0, 0, 0.0), |window| logical_extent(window));
        let adapter = self
            .gpu
            .as_ref()
            .map(|gpu| gpu.adapter_info())
            .unwrap_or_else(|| ("unknown".into(), "unknown".into()));
        format!(
            "macos_host={} adapter=\"{}\" backend={} width={} height={} scale={} frames={} activations={} tapped={} retired={} elapsed_ms={}",
            if self.error.is_none() { "pass" } else { "fail" },
            adapter.0,
            adapter.1,
            width,
            height,
            scale,
            self.frames,
            self.activations,
            self.tapped,
            self.closed,
            self.started.elapsed().as_millis()
        )
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.closed {
            return;
        }
        let result = self.resume(event_loop);
        if let Err(error) = result {
            self.error = Some(error.clone());
            stage("error", &error);
            event_loop.exit();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            return;
        }
        if let Err(error) = self.suspend() {
            self.error = Some(error.clone());
            stage("error", &error);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        let event = match event {
            AppEvent::Accessibility(event) => event,
            AppEvent::Exit => {
                stage("exit", "graceful host exit requested");
                self.shutdown();
                event_loop.exit();
                return;
            }
            AppEvent::Timeout => {
                stage("timeout", "host watchdog expired");
                self.error = Some("host watchdog expired before retirement".into());
                self.shutdown();
                event_loop.exit();
                return;
            }
        };
        stage("ax", &format!("event={:?}", event.window_event));
        match event.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => self.update_accessibility(),
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                let mut activated = false;
                if !self.engine.accesskit_action(
                    OWNER,
                    self.scene.version(),
                    self.generation,
                    &request,
                    &mut |_| {},
                    &mut |_event| {
                        activated = true;
                    },
                ) {
                    stage("accesskit", "action rejected");
                    return;
                }
                if activated {
                    let _ = self.activate();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
        }
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
        {
            let Self {
                accessibility,
                window,
                ..
            } = self;
            if let (Some(adapter), Some(window)) = (accessibility.as_mut(), window.as_ref()) {
                adapter.process_event(window, &event);
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                self.shutdown();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                stage("redraw", "requested");
                self.update_accessibility();
                if let Err(error) = self.render() {
                    self.error = Some(error.clone());
                    stage("error", &error);
                    event_loop.exit();
                    return;
                }
                let target = self.options.frames.or(if self.options.once {
                    Some(1)
                } else {
                    None
                });
                if target.is_some_and(|target| self.frames >= target) {
                    self.shutdown();
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Err(error) = self
                    .configure_surface()
                    .and_then(|()| self.reconfigure_engine())
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
        let target = self.options.frames.or(if self.options.once {
            Some(1)
        } else {
            None
        });
        let waiting = target.is_some_and(|target| self.frames < target) || self.options.tap;
        // The first frame is mandatory: an occluded layer has no drawable, so retry until the
        // window server hands one out instead of idling on a hidden surface.
        // 首帧是必须的：被遮挡的图层没有 drawable，因此持续重试，直到窗口服务器给出 drawable。
        let needs_first_frame = self.frames == 0;
        if self.window.is_some()
            && (waiting || needs_first_frame)
            && self.pending_surface > 0
            && self.pending_surface < 400
        {
            // Retry until the window server hands us a drawable; bounded so a hidden session
            // reports a missing surface instead of spinning forever.
            // 重试直到窗口服务器给出 drawable；有上限，使不可见会话报告缺 Surface 而不是无限自旋。
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + std::time::Duration::from_millis(50),
            ));
            if let Some(window) = &self.window {
                window.request_redraw();
                if needs_first_frame {
                    window.focus_window();
                }
            }
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {}

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.suspended {
            self.shutdown();
        }
    }
}

impl App {
    fn update_accessibility(&mut self) {
        let Some(adapter) = self.accessibility.as_mut() else {
            return;
        };
        let Some(tree) =
            self.engine
                .accessibility_tree(OWNER, self.scene.version(), self.generation)
        else {
            return;
        };
        let roles: Vec<String> = tree
            .nodes
            .iter()
            .map(|node| {
                format!(
                    "{:?}:{}:{}",
                    node.role,
                    node.label,
                    node.bounds
                        .map(|bounds| format!(
                            "{}x{}@{},{}",
                            bounds.size.width, bounds.size.height, bounds.origin.x, bounds.origin.y
                        ))
                        .unwrap_or_else(|| "none".into())
                )
            })
            .collect();
        stage(
            "ax",
            &format!("nodes={} focused={:?} tree=[{}]", tree.nodes.len(), tree.focused, roles.join(" ")),
        );
        // AccessKit bounds are physical pixels; the macOS adapter divides by the backing scale
        // factor. Scene bounds are logical, so the host scales them into physical coordinates.
        // AccessKit 的 bounds 是物理像素，macOS 适配器会除以 backing scale factor；
        // 场景 bounds 是逻辑坐标，因此由宿主缩放到物理坐标。
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor());
        let Some(update) = tree.to_accesskit(
            jasper_ui_engine::native::host::accesskit::Affine::scale(scale),
        ) else {
            stage("ax", "accesskit conversion rejected");
            return;
        };
        adapter.update_if_active(|| update);
    }

    fn resume(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Jasper UI · macOS")
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            self.options.size.0,
                            self.options.size.1,
                        ))
                        // The mockup draws its own traffic lights, so it hides the native chrome;
                        // the reference acceptance screen keeps the standard titled window.
                        // 原型界面自绘红绿灯，因此隐藏原生窗口装饰；验收界面保留标准带标题窗口。
                        .with_decorations(self.options.scene == SceneKind::Reference)
                        // The AccessKit adapter must exist before first visibility.
                        // AccessKit 适配器必须在首次可见之前建立。
                        .with_visible(false),
                )
                .map_err(|error| format!("macOS window creation failed: {error}"))?,
        );
        self.accessibility = Some(accesskit_winit::Adapter::with_event_loop_proxy(
            event_loop,
            &window,
            self.event_proxy.clone(),
        ));
        window.set_visible(true);
        // Bring the window to the front; an occluded layer has no drawable and cannot present.
        // 把窗口提到前面；被遮挡的图层没有 drawable，无法呈现。
        window.focus_window();
        let position = window
            .outer_position()
            .map(|position| format!("{:.0},{:.0}", position.x, position.y))
            .unwrap_or_else(|_| "unknown".into());
        stage(
            "window",
            &format!(
                "created pid={} position={position} logical={:.0}x{:.0}",
                std::process::id(),
                window.inner_size().width as f64 / window.scale_factor(),
                window.inner_size().height as f64 / window.scale_factor()
            ),
        );
        let (width, height, scale) = logical_extent(&window);
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| format!("macOS Metal surface: {error}"))?;
        let host = pollster::block_on(wgpu_context::DeviceHandle::new_from_compatible_surface(
            instance,
            Some(&surface),
            None,
            None,
        ))
        .map_err(|error| format!("wgpu macOS device: {error}"))?;
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
        .ok_or("no supported macOS surface format")?;
        let mut config = surface
            .get_default_config(&host.adapter, width, height)
            .ok_or("macOS surface configuration")?;
        config.format = format;
        config.present_mode = if self.options.immediate {
            wgpu::PresentMode::AutoNoVsync
        } else {
            wgpu::PresentMode::AutoVsync
        };
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
                .resume(OWNER, self.generation, width, height, scale)
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
        self.reconfigure_engine()?;
        let _ = self.engine.focus(
            OWNER,
            self.scene.version(),
            self.generation,
            BUTTON,
            &mut |_| {},
        );
        self.update_accessibility();
        window.request_redraw();
        Ok(())
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
    let unpadded = width
        .checked_mul(4)
        .ok_or("readback row overflow")?;
    let padded = unpadded.div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("macos-readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("macos-readback"),
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
        .map_err(|error| format!("macos readback poll: {error}"))?;
    receiver
        .recv()
        .map_err(|error| format!("macos readback channel: {error}"))?
        .map_err(|error| format!("macos readback map: {error}"))?;

    let data = slice
        .get_mapped_range()
        .map_err(|error| format!("macos readback view: {error}"))?;
    let bgra = matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );
    // Declared scene colors are linear RGB; the sRGB drawable stores their encoded form.
    // 场景声明色为线性 RGB；sRGB drawable 存的是编码后的值。
    let encode = |color: Color| {
        (
            srgb_encode(color.r),
            srgb_encode(color.g),
            srgb_encode(color.b),
        )
    };
    let (background, active, pressed) = (encode(BACKGROUND), encode(ACTIVE), encode(PRESSED));
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

fn main() {
    let (options, font) = match parse_args() {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let event_loop = match EventLoop::<AppEvent>::with_user_event()
        .with_activation_policy(ActivationPolicy::Regular)
        .build()
    {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("macOS event loop failed: {error}");
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
    let proxy = event_loop.create_proxy();
    let timeout_ms = options.timeout_ms;
    let exit_after_ms = options.exit_after_ms;
    let mut app = match App::new(options, &font, cjk.as_deref(), proxy) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("macOS host init failed: {error}");
            std::process::exit(2);
        }
    };
    if let Some(after) = exit_after_ms {
        // Graceful diagnostic exit: retire the native view, then leave the loop.
        // 诊断用优雅退出：先退休原生视图，再离开事件循环。
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(after));
            let _ = proxy.send_event(AppEvent::Exit);
        });
    }
    if let Some(timeout) = timeout_ms {
        // Host-owned watchdog: a headless run must not depend on a human closing the window.
        // 宿主自带的看门狗：无头运行不依赖人工关闭窗口。
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(timeout));
            let _ = proxy.send_event(AppEvent::Timeout);
        });
    }
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("macOS event loop failed: {error}");
        std::process::exit(2);
    }
    println!("{}", app.summary());
    for record in &app.records {
        println!(
            "readback path={} bytes={} background={} active={} pressed={} other={} sha256={}",
            record.path, record.bytes, record.background, record.active, record.pressed, record.other, record.sha256
        );
    }
    if let Some(error) = &app.error {
        eprintln!("macOS host error: {error}");
        std::process::exit(1);
    }
    if !app.closed {
        eprintln!("macOS host did not retire its native view");
        std::process::exit(1);
    }
}
