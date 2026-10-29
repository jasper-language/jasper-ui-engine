//! Font synthesis when the host injects only a regular face, as the pinned Blitz painter does it.
//! Two upstream gaps are pinned here so that any change is noticed: synthetic italic is drawn as a
//! backslant (the skew sign is mirrored in the y-down scene), and synthetic bold is not drawn at
//! all. Neither counts as delivered; docs/status.md records both.
//! 宿主只注入常规字面时，固定版 Blitz 绘制器的合成行为。此处钉住两个上游缺口以便察觉变化：
//! 合成斜体画成反斜体（y 向下场景中倾斜符号取反），合成粗体完全不画。两者均不计为已交付，
//! docs/status.md 如实记录。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,FontStyle,Length,Style};

const W:u32=320;
const H:u32=80;
const OWNER:u64=9;
const BUDGET:usize=4*1024*1024;
const ROOT_FILL:[u8;4]=[18,22,30,255];
const TEXT:NodeId=NodeId(2);
const SAMPLE:&str="Jasper IIII llll";

/// Injected font files; the test never scans the system.
/// 注入的字体文件；测试不扫描系统字体。
fn regular()->Vec<u8> {read("JASPER_UI_TEST_LATIN_FONT")}
fn italic()->Vec<u8> {read("JASPER_UI_TEST_ITALIC_FONT")}
fn read(var:&str)->Vec<u8> {
    let path=std::env::var(var).unwrap_or_else(|_|panic!("{var} must name an injected font file"));
    std::fs::read(path).expect("injected font readable")
}

fn text_style(weight:u16,slant:FontStyle)->Style {
    Style::default().width(Length::Px(280.0)).text(28.0,Color::WHITE).font(weight,slant)
}

/// The padding keeps a leaning glyph off the left edge so its full outline is drawn.
/// 内边距使倾斜字形不贴左缘，完整绘出。
fn scene_of(text:&str,weight:u16,slant:FontStyle)->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).padding(Edges::all(20.0)).background(Color::rgb(18,22,30)))
        .child(Node::text(TEXT,text).with_style(text_style(weight,slant))))
}

fn render(engine:&mut Engine,version:u64)->Vec<u8> {
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
    bytes
}

fn frame_of(font:&[u8],text:&str,weight:u16,slant:FontStyle)->Vec<u8> {
    let source=scene_of(text,weight,slant);
    let mut engine=Engine::with_fonts(OWNER,&source,&[font],W,H).expect("native import");
    render(&mut engine,source.version())
}

fn frame(font:&[u8],weight:u16,slant:FontStyle)->Vec<u8> {frame_of(font,SAMPLE,weight,slant)}

fn is_ink(p:&[u8])->bool {p!=ROOT_FILL}
fn ink(bytes:&[u8])->usize {bytes.chunks_exact(4).filter(|p|is_ink(p)).count()}

/// Leftmost ink column of each inked row of a lone "I" stem, top row first.
/// 单个 "I" 竖笔画每个有墨迹行的最左墨迹列，自上而下。
fn stem_edges(font:&[u8],slant:FontStyle)->Vec<u32> {
    let bytes=frame_of(font,"I",400,slant);
    (0..H).filter_map(|y|(0..W).find(|x| {
        let at=((y*W+x)*4) as usize;
        is_ink(&bytes[at..at+4])
    })).collect()
}

/// Top-row edge minus bottom-row edge: positive leans right like an italic, negative leans back.
/// 顶行左缘减底行左缘：正值如斜体右倾，负值为反倾。
fn lean(edges:&[u32])->i64 {*edges.first().expect("inked stem") as i64-*edges.last().expect("inked stem") as i64}

#[test]
fn synthetic_italic_from_the_pinned_painter_leans_backwards() {
    let (regular,oblique)=(regular(),italic());
    let (slanted,upright)=(frame(&regular,400,FontStyle::Italic),frame(&regular,400,FontStyle::Normal));
    assert_ne!(slanted,upright,"italic over a regular-only stack is synthesised, not drawn upright");
    let (slanted_ink,upright_ink)=(ink(&slanted),ink(&upright));
    println!("ink pixels: synthetic italic {slanted_ink}, upright {upright_ink}");
    assert!(slanted_ink*10>=upright_ink*9 && slanted_ink*10<=upright_ink*11,"{slanted_ink} vs {upright_ink}");
    // Direction anchor: the upright stem is vertical and the injected oblique face leans right.
    // 方向锚点：直立竖笔画垂直，注入的真斜体字面右倾。
    let (upright_edges,oblique_edges,synthetic_edges)=(
        stem_edges(&regular,FontStyle::Normal),stem_edges(&oblique,FontStyle::Italic),stem_edges(&regular,FontStyle::Italic));
    println!("stem lean: upright {}, oblique face {}, synthetic {}",lean(&upright_edges),lean(&oblique_edges),lean(&synthetic_edges));
    assert!(lean(&upright_edges).abs()<=1,"upright stem must be vertical: {upright_edges:?}");
    assert!(lean(&oblique_edges)>=3,"oblique face must lean right: {oblique_edges:?}");
    // The pinned painter's synthetic skew leans the other way, monotonically: a known defect.
    // 固定版绘制器的合成倾斜单调地朝反方向：已知缺陷。
    assert!(lean(&synthetic_edges)<=-3,"synthetic italic lean changed: {synthetic_edges:?}; update docs/status.md");
    assert!(synthetic_edges.windows(2).all(|pair|pair[0]<=pair[1]),"{synthetic_edges:?}");
    // A style patch to Italic repaints the same synthetic frame as a fresh import.
    // 改为 Italic 的样式补丁重绘结果等于新导入的合成帧。
    let source=scene_of(SAMPLE,400,FontStyle::Normal);
    let v=source.version();
    let mut engine=Engine::with_fonts(OWNER,&source,&[&regular],W,H).expect("native import");
    assert_eq!(render(&mut engine,v),upright);
    assert!(engine.apply(OWNER,v,v+1,&[ScenePatch::UpdateStyle {id:TEXT,style:text_style(400,FontStyle::Italic)}]));
    assert_eq!(engine.needs_frame(OWNER),Some(true));
    assert_eq!(render(&mut engine,v+1),slanted);
}

#[test]
fn bold_without_a_bold_face_draws_the_regular_face() {
    // The pinned painter ignores Parley's embolden synthesis (only a global embolden feature
    // exists, left off), so weight 600..900 over a regular-only stack equals weight 400.
    // 固定版绘制器忽略 Parley 的加粗合成（只有全局加粗特性，未开启），故只注入常规字面时
    // 字重 600..900 等于 400。
    let regular=regular();
    let upright=frame(&regular,400,FontStyle::Normal);
    for weight in [600,700,900] {
        assert_eq!(frame(&regular,weight,FontStyle::Normal),upright,"weight {weight}: bold synthesis changed; update docs/status.md");
    }
    // Synthetic italic is unaffected by the requested weight.
    // 合成斜体不受所请求字重影响。
    assert_eq!(frame(&regular,700,FontStyle::Italic),frame(&regular,400,FontStyle::Italic));
}
