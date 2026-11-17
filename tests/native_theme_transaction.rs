//! P0-A atomic theme switching across the three component families. The engine consumes only
//! engine-neutral style frames; business node identity, focus, edit/IME buffer, scroll and
//! accessibility identity survive every palette swap. A rejected palette leaves the old frame.
//! P0-A 三组件家族的主题原子切换；引擎只消费中立样式帧，业务节点身份、焦点、编辑/IME、
//! 滚动与无障碍身份在每次切换后保持。被拒主题整帧回退。
#![cfg(feature = "native-engine-cpu")]

use jasper_ui_engine::geometry::Point;
use jasper_ui_engine::native::SceneFocusEvent;
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{HeadingLevel, Role, Semantics};
use jasper_ui_engine::style::{Color, Length, Overflow, Style};
use jasper_ui_engine::Engine;

const OWNER: u64 = 501;
const W: u32 = 320;
const H: u32 = 220;
const ROOT: NodeId = NodeId(1);
const TITLE: NodeId = NodeId(2);
const FIELD: NodeId = NodeId(3);
const BUTTON: NodeId = NodeId(4);
const SCROLLER: NodeId = NodeId(5);
const LONG: NodeId = NodeId(6);

#[derive(Clone, Copy)]
struct Palette {
    root: Color,
    surface: Color,
    text: Color,
    accent: Color,
    field: Color,
}

const APPLE: Palette = Palette {
    root: Color::rgb(246, 246, 248),
    surface: Color::rgb(255, 255, 255),
    text: Color::rgb(22, 22, 24),
    accent: Color::rgb(0, 122, 255),
    field: Color::rgb(238, 238, 242),
};
const ANT: Palette = Palette {
    root: Color::rgb(245, 245, 245),
    surface: Color::rgb(255, 255, 255),
    text: Color::rgb(38, 38, 38),
    accent: Color::rgb(22, 119, 255),
    field: Color::rgb(250, 250, 250),
};
const SHADCN: Palette = Palette {
    root: Color::rgb(250, 250, 250),
    surface: Color::rgb(255, 255, 255),
    text: Color::rgb(10, 10, 10),
    accent: Color::rgb(24, 24, 27),
    field: Color::rgb(244, 244, 245),
};

fn styles(p: Palette) -> Vec<(NodeId, Style)> {
    vec![
        (
            ROOT,
            Style::default()
                .size(Length::Px(W as f32), Length::Px(H as f32))
                .background(p.root)
                .gap(8.0),
        ),
        (
            TITLE,
            Style::default()
                .size(Length::Px(280.0), Length::Px(24.0))
                .background(p.surface)
                .text(18.0, p.text),
        ),
        (
            FIELD,
            Style::default()
                .size(Length::Px(280.0), Length::Px(32.0))
                .background(p.field)
                .text(14.0, p.text),
        ),
        (
            BUTTON,
            Style::default()
                .size(Length::Px(120.0), Length::Px(32.0))
                .background(p.accent)
                .text(14.0, Color::WHITE),
        ),
        (
            SCROLLER,
            Style::default()
                .size(Length::Px(280.0), Length::Px(80.0))
                .overflow(Overflow::Scroll)
                .background(p.surface),
        ),
    ]
}

fn scene(p: Palette) -> Scene {
    let mut all = styles(p).into_iter();
    let mut long_style = Style::default()
        .size(Length::Px(280.0), Length::Px(180.0))
        .background(Color::WHITE)
        .text(14.0, Color::BLACK);
    long_style.min_height = Length::Px(180.0);
    Scene::new(
        Node::container(ROOT)
            .with_style(all.next().unwrap().1)
            .child(
                Node::text(TITLE, "Settings")
                    .with_style(all.next().unwrap().1)
                    .with_semantics(
                        Semantics::default()
                            .role(Role::Heading(HeadingLevel::new(1).unwrap()))
                            .label("Settings")
                            .focusable(false),
                    ),
            )
            .child(
                Node::text(FIELD, "Name")
                    .with_style(all.next().unwrap().1)
                    .with_semantics(
                        Semantics::default()
                            .role(Role::TextField)
                            .label("Name")
                            .focusable(true),
                    ),
            )
            .child(
                Node::text(BUTTON, "Save")
                    .with_style(all.next().unwrap().1)
                    .with_semantics(
                        Semantics::default()
                            .role(Role::Button)
                            .label("Save")
                            .focusable(true),
                    ),
            )
            .child(
                Node::container(SCROLLER)
                    .with_style(all.next().unwrap().1)
                    .child(
                        Node::text(LONG, "A long scrollable settings body")
                            .with_style(long_style),
                    ),
            ),
    )
}

fn render(engine: &mut Engine, version: u64) -> Vec<u8> {
    let mut frame = vec![0u8; (W * H * 4) as usize];
    assert!(engine.render_to_surface(
        OWNER,
        version,
        &mut frame,
        (W * 4) as usize,
        1 << 24
    ));
    frame
}

fn apply_palette(live: &mut Engine, source: &mut Scene, palette: Palette) {
    let patches: Vec<_> = styles(palette)
        .into_iter()
        .map(|(id, style)| ScenePatch::UpdateStyle { id, style })
        .collect();
    let old = source.version();
    source.apply_batch_at(old, &patches).unwrap();
    let mut focus = Vec::<SceneFocusEvent>::new();
    assert!(live.apply_with_focus_events(
        OWNER,
        old,
        source.version(),
        &patches,
        &mut |event| focus.push(event),
    ));
    assert!(focus.is_empty());
}

#[test]
fn three_palette_cycles_preserve_business_and_input_identity() {
    let font = std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("test font")).unwrap();
    let mut source = scene(APPLE);
    let mut live = Engine::new(OWNER, &source, &font, W, H).unwrap();
    assert!(live.focus(OWNER, source.version(), FIELD, &mut |_| {}));
    assert!(live.text_input(OWNER, source.version(), "q"));
    assert!(live.set_preedit(
        OWNER,
        source.version(),
        Some("中"),
        Some((0, "中".len()))
    ));
    let scroll = ScenePatch::SetScroll {
        id: SCROLLER,
        offset: Point::new(0.0, 12.0),
    };
    let old = source.version();
    source.apply_batch_at(old, &[scroll.clone()]).unwrap();
    assert!(live.apply(OWNER, old, source.version(), &[scroll]));
    let edit = live
        .edit_state(OWNER, source.version())
        .unwrap()
        .unwrap();
    assert_eq!(edit.text, "Nameq");
    assert_eq!(edit.preedit.as_deref(), Some("中"));
    let first_tree = live.accessibility_tree(OWNER, source.version()).unwrap();
    let ids: Vec<NodeId> = first_tree.nodes.iter().map(|node| node.node).collect();
    let first_apple = render(&mut live, source.version());

    for palette in [ANT, SHADCN, APPLE, ANT, SHADCN, APPLE] {
        apply_palette(&mut live, &mut source, palette);
        assert_eq!(live.focused_node(OWNER, source.version()), Some(FIELD));
        let edit = live
            .edit_state(OWNER, source.version())
            .unwrap()
            .unwrap();
        assert_eq!(edit.text, "Nameq");
        assert_eq!(edit.preedit.as_deref(), Some("中"));
        assert_eq!(
            live.scroll_geometry(OWNER, source.version(), SCROLLER)
                .unwrap()
                .offset,
            Point::new(0.0, 12.0)
        );
        let tree = live.accessibility_tree(OWNER, source.version()).unwrap();
        assert_eq!(
            tree.nodes.iter().map(|node| node.node).collect::<Vec<_>>(),
            ids
        );
        assert_eq!(tree.focused, Some(FIELD));
    }
    assert_eq!(render(&mut live, source.version()), first_apple);

    let old = source.version();
    let bad = [ScenePatch::UpdateStyle {
        id: BUTTON,
        style: {
            let mut style = styles(ANT)[3].1.clone();
            style.gap = f32::NAN;
            style
        },
    }];
    assert!(live
        .try_apply(OWNER, old, old + 1, &bad)
        .is_err());
    assert_eq!(live.focused_node(OWNER, old), Some(FIELD));
    assert_eq!(render(&mut live, old), first_apple);
}
