//! Font synthesis when the host injects only a regular face. Synthetic italic is drawn leaning
//! right like a real italic: the pinned Blitz painter mirrors the skew in the y-down scene, and
//! the engine corrects the sign on every glyph run. Synthetic bold uses a weight-700 stand-in
//! registration of the regular face and emboldens only glyph runs from that registered blob.
//! 宿主只注入常规字面时的字体合成。合成斜体如真斜体般右倾：固定版 Blitz 绘制器在 y 向下场景中
//! 把斜切符号取反，引擎在每个字形串上修正符号。合成粗体通过把常规字面以字重 700 再登记为
//! 替身 Blob，绘制器只对来自该 Blob 的字形串加粗。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::style::{Color, FontStyle, Length, Style};

const W: u32 = 320;
const H: u32 = 80;
const OWNER: u64 = 9;
const BUDGET: usize = 4 * 1024 * 1024;
const ROOT_FILL: [u8; 4] = [18, 22, 30, 255];
const TEXT: NodeId = NodeId(2);
const SAMPLE: &str = "Jasper IIII llll";

/// Injected font files; the test never scans the system.
/// 注入的字体文件；测试不扫描系统字体。
fn regular() -> Vec<u8> {
    read("JASPER_UI_TEST_LATIN_FONT")
}
fn italic() -> Vec<u8> {
    read("JASPER_UI_TEST_ITALIC_FONT")
}
fn bold() -> Vec<u8> {
    read("JASPER_UI_TEST_BOLD_FONT")
}
fn read(var: &str) -> Vec<u8> {
    let path =
        std::env::var(var).unwrap_or_else(|_| panic!("{var} must name an injected font file"));
    std::fs::read(path).expect("injected font readable")
}

fn text_style(weight: u16, slant: FontStyle) -> Style {
    Style::default()
        .width(Length::Px(280.0))
        .text(28.0, Color::WHITE)
        .font(weight, slant)
}

/// The padding keeps a leaning glyph off the left edge so its full outline is drawn.
/// 内边距使倾斜字形不贴左缘，完整绘出。
fn scene_of(text: &str, weight: u16, slant: FontStyle) -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .padding(Edges::all(20.0))
                    .background(Color::rgb(18, 22, 30)),
            )
            .child(Node::text(TEXT, text).with_style(text_style(weight, slant))),
    )
}

fn render(engine: &mut Engine, version: u64) -> Vec<u8> {
    let mut bytes = vec![0; (W * H * 4) as usize];
    assert!(engine.render_to_surface(OWNER, version, &mut bytes, (W * 4) as usize, BUDGET));
    bytes
}

fn frame_of(font: &[u8], text: &str, weight: u16, slant: FontStyle) -> Vec<u8> {
    let source = scene_of(text, weight, slant);
    let mut engine = Engine::with_fonts(OWNER, &source, &[font], W, H).expect("native import");
    render(&mut engine, source.version())
}

fn frame(font: &[u8], weight: u16, slant: FontStyle) -> Vec<u8> {
    frame_of(font, SAMPLE, weight, slant)
}

fn is_ink(p: &[u8]) -> bool {
    p != ROOT_FILL
}
fn ink(bytes: &[u8]) -> usize {
    bytes.chunks_exact(4).filter(|p| is_ink(p)).count()
}

/// Leftmost ink column of each inked row of a lone "I" stem, top row first.
/// 单个 "I" 竖笔画每个有墨迹行的最左墨迹列，自上而下。
fn stem_edges(font: &[u8], slant: FontStyle) -> Vec<u32> {
    let bytes = frame_of(font, "I", 400, slant);
    (0..H)
        .filter_map(|y| {
            (0..W).find(|x| {
                let at = ((y * W + x) * 4) as usize;
                is_ink(&bytes[at..at + 4])
            })
        })
        .collect()
}

/// Top-row edge minus bottom-row edge: positive leans right like an italic, negative leans back.
/// 顶行左缘减底行左缘：正值如斜体右倾，负值为反倾。
fn lean(edges: &[u32]) -> i64 {
    *edges.first().expect("inked stem") as i64 - *edges.last().expect("inked stem") as i64
}

#[test]
fn synthetic_italic_leans_right_like_an_italic_face() {
    let (regular, oblique) = (regular(), italic());
    let (slanted, upright) = (
        frame(&regular, 400, FontStyle::Italic),
        frame(&regular, 400, FontStyle::Normal),
    );
    assert_ne!(
        slanted, upright,
        "italic over a regular-only stack is synthesised, not drawn upright"
    );
    let (slanted_ink, upright_ink) = (ink(&slanted), ink(&upright));
    println!("ink pixels: synthetic italic {slanted_ink}, upright {upright_ink}");
    assert!(
        slanted_ink * 10 >= upright_ink * 9 && slanted_ink * 10 <= upright_ink * 11,
        "{slanted_ink} vs {upright_ink}"
    );
    // Direction anchor: the upright stem is vertical and the injected oblique face leans right.
    // 方向锚点：直立竖笔画垂直，注入的真斜体字面右倾。
    let (upright_edges, oblique_edges, synthetic_edges) = (
        stem_edges(&regular, FontStyle::Normal),
        stem_edges(&oblique, FontStyle::Italic),
        stem_edges(&regular, FontStyle::Italic),
    );
    println!(
        "stem lean: upright {}, oblique face {}, synthetic {}",
        lean(&upright_edges),
        lean(&oblique_edges),
        lean(&synthetic_edges)
    );
    assert!(
        lean(&upright_edges).abs() <= 1,
        "upright stem must be vertical: {upright_edges:?}"
    );
    assert!(
        lean(&oblique_edges) >= 3,
        "oblique face must lean right: {oblique_edges:?}"
    );
    // The corrected synthetic skew leans the same way as the real face, monotonically.
    // 修正后的合成倾斜与真斜体字面同向，且单调。
    assert!(
        lean(&synthetic_edges) >= 3,
        "synthetic italic must lean right: {synthetic_edges:?}"
    );
    assert!(
        synthetic_edges.windows(2).all(|pair| pair[0] >= pair[1]),
        "{synthetic_edges:?}"
    );
    // A style patch to Italic repaints the same synthetic frame as a fresh import.
    // 改为 Italic 的样式补丁重绘结果等于新导入的合成帧。
    let source = scene_of(SAMPLE, 400, FontStyle::Normal);
    let v = source.version();
    let mut engine = Engine::with_fonts(OWNER, &source, &[&regular], W, H).expect("native import");
    assert_eq!(render(&mut engine, v), upright);
    assert!(engine.apply(
        OWNER,
        v,
        v + 1,
        &[ScenePatch::UpdateStyle {
            id: TEXT,
            style: text_style(400, FontStyle::Italic)
        }]
    ));
    assert_eq!(engine.needs_frame(OWNER), Some(true));
    assert_eq!(render(&mut engine, v + 1), slanted);
}

#[test]
fn bold_without_a_bold_face_uses_the_registered_standin() {
    // The pinned painter does not request Parley's embolden synthesis. The font resource layer
    // registers a weight-700 copy of the regular blob; CSS matching selects it only for bold
    // requests and the forwarding painter emboldens only that blob.
    // 固定版绘制器不请求 Parley 加粗合成；字体资源层把常规 Blob 以字重 700 再登记，
    // CSS 匹配仅在粗体请求时选中替身，转发绘制器只加粗该 Blob。
    let regular = regular();
    let upright = frame(&regular, 400, FontStyle::Normal);
    let synthesized = frame(&regular, 700, FontStyle::Normal);
    assert_ne!(
        synthesized, upright,
        "bold stand-in must change the painted outlines"
    );
    assert!(
        ink(&synthesized) > ink(&upright),
        "bold stand-in must add ink"
    );
    for weight in [600, 900] {
        assert_eq!(
            frame(&regular, weight, FontStyle::Normal),
            synthesized,
            "weight {weight}: the weight-700 stand-in must serve all bold requests"
        );
    }
    let italic = frame(&regular, 400, FontStyle::Italic);
    let bold_italic = frame(&regular, 700, FontStyle::Italic);
    assert_ne!(bold_italic, italic);
    assert_ne!(bold_italic, synthesized);
    assert!(ink(&bold_italic) > ink(&italic));
}

#[test]
fn real_bold_face_is_preferred_over_the_regular_standin() {
    let (regular, bold) = (regular(), bold());
    let source = scene_of(SAMPLE, 700, FontStyle::Normal);
    let mut standalone = Engine::with_fonts(OWNER, &source, &[&bold], W, H).expect("bold import");
    let mut combined =
        Engine::with_fonts(OWNER, &source, &[&regular, &bold], W, H).expect("font stack");
    assert_eq!(
        render(&mut standalone, source.version()),
        render(&mut combined, source.version()),
        "real bold face must win over the regular synthetic stand-in"
    );
}

/// The GPU path paints through the same corrected glyph runs: its synthetic stem leans right too.
/// GPU 路径经同一修正后的字形串绘制：其合成竖笔画同样右倾。
#[cfg(feature = "gpu-pending")]
#[test]
fn gpu_synthetic_italic_leans_right_too() {
    use jasper_ui_engine::native::host::GpuRenderer;
    let regular = regular();
    let source = scene_of("I", 400, FontStyle::Italic);
    let mut engine = Engine::with_fonts(OWNER, &source, &[&regular], W, H).expect("native import");
    let mut gpu =
        GpuRenderer::new_headless().expect("a wgpu adapter is required for GPU acceptance");
    let bytes = engine
        .render_gpu(OWNER, source.version(), &mut gpu, BUDGET)
        .expect("gpu frame");
    let edges: Vec<u32> = (0..H)
        .filter_map(|y| {
            (0..W).find(|x| {
                let at = ((y * W + x) * 4) as usize;
                is_ink(&bytes[at..at + 4])
            })
        })
        .collect();
    println!("gpu synthetic stem lean {}", lean(&edges));
    assert!(
        lean(&edges) >= 3,
        "gpu synthetic italic must lean right: {edges:?}"
    );
}

#[cfg(feature = "gpu-pending")]
#[test]
fn gpu_regular_only_stack_uses_the_bold_standin_too() {
    use jasper_ui_engine::native::host::GpuRenderer;
    let regular = regular();
    let render_gpu = |weight| {
        let source = scene_of(SAMPLE, weight, FontStyle::Normal);
        let mut engine =
            Engine::with_fonts(OWNER, &source, &[&regular], W, H).expect("native import");
        let mut gpu =
            GpuRenderer::new_headless().expect("a wgpu adapter is required for GPU acceptance");
        engine
            .render_gpu(OWNER, source.version(), &mut gpu, BUDGET)
            .expect("gpu frame")
    };
    let (plain, bold) = (render_gpu(400), render_gpu(700));
    assert_ne!(
        plain, bold,
        "GPU bold stand-in must change painted outlines"
    );
    assert!(ink(&bold) > ink(&plain), "GPU bold stand-in must add ink");
}
