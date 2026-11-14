//! E15 text vertical alignment and line height on the native CPU path: xstyle parsing,
//! default pixel identity, equal top/bottom space when centred (±1 px), bottom alignment,
//! row/block/padding/radius/transform cooperation, layout/hit/node_bounds invariance.
//! 原生 CPU 路径上的 E15 文字垂直对齐与行高：xstyle 解析、默认逐像素一致、居中时上下留白相等（±1px）、底对齐、
//! 与行方向/块/内边距/圆角/变换协同、布局/命中/node_bounds 不变。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::{Edges, Rect};
use jasper_ui_engine::native::host::style_capabilities;
use jasper_ui_engine::native::{EventReply, PointerAction, SceneEventKind};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{
    Color, Display, FlexDirection, LineHeight, Length, Style, TextAlignVertical,
    Transform2D,
};

const OWNER: u64 = 15;
const W: u32 = 240;
const H: u32 = 120;
const TEXT: NodeId = NodeId(2);
const BOX_H: f32 = 60.0;

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT")).unwrap()
}

fn text_style() -> Style {
    Style::default()
        .size(Length::Px(200.0), Length::Px(BOX_H))
        .text(16.0, Color::BLACK)
}

fn scene(style: Style, text: &str) -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .background(Color::WHITE),
            )
            .child(
                Node::text(TEXT, text)
                    .with_style(style)
                    .with_semantics(Semantics::default().role(Role::Button).label("ok")),
            ),
    )
}

fn engine(s: &Scene) -> Engine {
    Engine::with_fonts(OWNER, s, &[&font()], W, H).expect("native import")
}

fn render(e: &mut Engine, version: u64) -> Vec<u8> {
    let mut bytes = vec![0u8; (W * H * 4) as usize];
    assert!(e.render_to_surface(OWNER, version, &mut bytes, (W * 4) as usize, 1 << 24));
    bytes
}

/// Rows holding text ink (any channel darker than 160) inside x < 200.
fn ink_rows(frame: &[u8]) -> Option<(u32, u32)> {
    let mut rows = (0..H).filter(|&y| {
        (0..200).any(|x| {
            let i = ((y * W + x) * 4) as usize;
            frame[i] < 160
        })
    });
    let first = rows.next()?;
    Some((first, rows.last().unwrap_or(first)))
}

/// Line box of the text (caret at offset 0) and the node box.
fn line_and_box(e: &Engine, version: u64) -> (Rect, Rect) {
    (
        e.text_caret(OWNER, version, TEXT, 0, 1.0).expect("caret"),
        e.node_bounds(OWNER, version, TEXT).expect("bounds"),
    )
}

#[test]
fn capability_and_xstyle_grammar() {
    let cap = style_capabilities()
        .iter()
        .find(|c| c.name == "text-valign")
        .expect("text-valign capability");
    assert_eq!(cap.xstyle_keys, &["text-valign", "line-height"]);
    let mut s = Style::default();
    s.apply_xstyle("text-valign", "center").unwrap();
    s.apply_xstyle("line-height", "20px").unwrap();
    assert_eq!(s.text_align_vertical, Some(TextAlignVertical::Center));
    assert_eq!(s.line_height, Some(LineHeight::Px(20.0)));
    s.apply_xstyle("line-height", "1.5").unwrap();
    assert_eq!(s.line_height, Some(LineHeight::Multiple(1.5)));
    let pairs = s.xstyle_pairs();
    assert!(pairs.contains(&("text-valign", "center".to_owned())));
    assert!(pairs.contains(&("line-height", "1.5".to_owned())));
    let mut back = Style::default();
    for (k, v) in &pairs {
        back.apply_xstyle(k, v).unwrap();
    }
    assert_eq!(back, s);
    for (k, v) in [
        ("text-valign", "middle"),
        ("text-valign", ""),
        ("line-height", "0"),
        ("line-height", "-2px"),
        ("line-height", "17"),
        ("line-height", "5000px"),
        ("line-height", "20em"),
        ("line-height", "nan"),
    ] {
        let before = s.clone();
        assert!(s.apply_xstyle(k, v).is_err(), "{k}={v}");
        assert_eq!(s, before);
    }
    s.apply_xstyle("text-valign", "none").unwrap();
    s.apply_xstyle("line-height", "none").unwrap();
    assert_eq!(s, Style::default());
}

#[test]
fn defaults_and_explicit_top_are_pixel_identical() {
    let base = scene(text_style(), "Hg Ok");
    let reference = render(&mut engine(&base), base.version());
    let top = scene(
        text_style().text_align_vertical(Some(TextAlignVertical::Top)),
        "Hg Ok",
    );
    assert_eq!(render(&mut engine(&top), top.version()), reference);
    // Valign on a non-text node changes nothing either.
    let boxed = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .background(Color::WHITE)
                    .text_align_vertical(Some(TextAlignVertical::Bottom))
                    .line_height(Some(LineHeight::Px(40.0))),
            )
            .child(
                Node::text(TEXT, "Hg Ok")
                    .with_style(text_style())
                    .with_semantics(Semantics::default().role(Role::Button).label("ok")),
            ),
    );
    assert_eq!(render(&mut engine(&boxed), boxed.version()), reference);
}

#[test]
fn centred_text_has_equal_space_above_and_below() {
    let top = scene(text_style(), "Hg Ok");
    let mut te = engine(&top);
    let top_ink = ink_rows(&render(&mut te, top.version())).unwrap();
    let (top_line, top_box) = line_and_box(&te, top.version());
    assert!((top_line.y() - top_box.y()).abs() <= 1.0);

    let c = scene(
        text_style().text_align_vertical(Some(TextAlignVertical::Center)),
        "Hg Ok",
    );
    let mut ce = engine(&c);
    let frame = render(&mut ce, c.version());
    let (line, b) = line_and_box(&ce, c.version());
    assert_eq!(b, top_box, "node_bounds unchanged");
    let above = line.y() - b.y();
    let below = (b.y() + b.height()) - (line.y() + line.height());
    assert!((above - below).abs() <= 1.0, "above {above} below {below}");
    // Pixels: the ink moved down by the same amount as the line box.
    let ink = ink_rows(&frame).unwrap();
    let shift = ink.0 as f32 - top_ink.0 as f32;
    assert!((shift - above).abs() <= 1.0, "ink shift {shift} vs {above}");
    assert_eq!(ink.1 - ink.0, top_ink.1 - top_ink.0);

    let bottom = scene(
        text_style().text_align_vertical(Some(TextAlignVertical::Bottom)),
        "Hg Ok",
    );
    let be = engine(&bottom);
    let (line, b) = line_and_box(&be, bottom.version());
    assert!(((b.y() + b.height()) - (line.y() + line.height())).abs() <= 1.0);
}

#[test]
fn multi_line_line_height_and_padding() {
    // Two wrapped lines with a 24px line height, centred inside 20px padding.
    let style = Style::default()
        .size(Length::Px(60.0), Length::Px(100.0))
        .padding(Edges::all(10.0))
        .text(16.0, Color::BLACK)
        .line_height(Some(LineHeight::Px(24.0)))
        .text_align_vertical(Some(TextAlignVertical::Center));
    let s = scene(style, "Hgg Okk");
    let e = engine(&s);
    let first = e.text_caret(OWNER, s.version(), TEXT, 0, 1.0).unwrap();
    let last = e.text_caret(OWNER, s.version(), TEXT, "Hgg Okk".len(), 1.0).unwrap();
    assert!((first.height() - 24.0).abs() <= 1.0, "line height {}", first.height());
    assert!(last.y() > first.y() + 20.0, "two lines");
    let b = e.node_bounds(OWNER, s.version(), TEXT).unwrap();
    assert_eq!((b.width(), b.height()), (60.0, 100.0));
    // Content box 80px tall, text 48px: 16px above and below inside the padding.
    let above = first.y() - (b.y() + 10.0);
    let below = (b.y() + 90.0) - (last.y() + last.height());
    assert!((above - below).abs() <= 1.0, "above {above} below {below}");
    assert!((above - 16.0).abs() <= 1.0);
    // Multiple: 2 × 16px.
    let m = scene(
        text_style().line_height(Some(LineHeight::Multiple(2.0))),
        "Hg",
    );
    let me = engine(&m);
    let caret = me.text_caret(OWNER, m.version(), TEXT, 0, 1.0).unwrap();
    assert!((caret.height() - 32.0).abs() <= 1.0, "{}", caret.height());
}

#[test]
fn row_and_block_text_nodes_centre_too() {
    for (display, direction) in [
        (Display::Flex, FlexDirection::Row),
        (Display::Block, FlexDirection::Column),
    ] {
        let s = scene(
            text_style()
                .display(display)
                .direction(direction)
                .text_align_vertical(Some(TextAlignVertical::Center)),
            "Hg",
        );
        let e = engine(&s);
        let (line, b) = line_and_box(&e, s.version());
        let above = line.y() - b.y();
        let below = (b.y() + b.height()) - (line.y() + line.height());
        assert!((above - below).abs() <= 1.0, "{display:?} {direction:?}: {above} {below}");
    }
}

#[test]
fn radius_transform_hit_and_update_cooperate() {
    let style = text_style()
        .background(Color::rgb(230, 230, 230))
        .corner_radius(12.0)
        .text_align_vertical(Some(TextAlignVertical::Center));
    let s = scene(style.clone(), "Hg");
    let mut e = engine(&s);
    let frame = render(&mut e, s.version());
    // Rounded corner still cut (white), centre ink present.
    let px = |x: u32, y: u32| frame[((y * W + x) * 4) as usize];
    assert_eq!(px(0, 0), 255);
    assert!(ink_rows(&frame).unwrap().0 > 10);
    // The whole node box stays the hit target (top strip above the centred text).
    let mut clicks = 0;
    for phase in [PointerAction::Down, PointerAction::Up] {
        assert!(e.pointer(OWNER, s.version(), phase, 100.0, 3.0, phase == PointerAction::Down,
            &mut |ev| {
                if ev.kind == SceneEventKind::Activate && ev.node == TEXT {
                    clicks += 1;
                }
                EventReply::default()
            },
            &mut |_| {}));
    }
    assert_eq!(clicks, 1);
    // A transform moves line box and node together in paint; node_bounds stays the layout box.
    let moved = style.transform(Transform2D::translate(10.0, 5.0));
    assert!(e.apply(OWNER, s.version(), s.version() + 1, &[ScenePatch::UpdateStyle {
        id: TEXT,
        style: moved,
    }]));
    let (line, b) = line_and_box(&e, s.version() + 1);
    assert_eq!((b.x(), b.y()), (0.0, 0.0));
    let above = line.y() - b.y();
    assert!((above - (b.height() - line.height()) / 2.0).abs() <= 1.0);
}
