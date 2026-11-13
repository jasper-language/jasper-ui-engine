//! E2 corner radius, E3 shadow, E4 opacity, E9 border sides / outline / text decoration and
//! E10 hit slop on the native CPU path: headless pixel assertions, layout invariance,
//! hit testing and unchanged accessibility.
//! 原生 CPU 路径上的 E2 圆角、E3 阴影、E4 透明度、E9 单侧边框/外框/文字装饰与 E10 命中扩展：
//! 无头出图像素断言、布局不变、命中测试与无障碍不变。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::host::style_capabilities;
use jasper_ui_engine::native::{
    EventReply, PointerAction, SceneEventKind, SceneUpdate, SceneUpdateError,
};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{
    BorderSide, BoxShadow, Color, CornerRadii, Display, FlexDirection, Length, Outline, Overflow,
    Side, Style, TextDecoration,
};

const OWNER: u64 = 7;
const W: u32 = 160;
const H: u32 = 120;
const BG: [u8; 4] = [255, 255, 255, 255];

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT")).unwrap()
}

/// White W×H root with 20px padding; children flow in a column (or a row) from (20, 20).
fn root(direction: FlexDirection) -> Node {
    Node::container(NodeId(1)).with_style(
        Style::default()
            .size(Length::Px(W as f32), Length::Px(H as f32))
            .padding(Edges::all(20.0))
            .gap(10.0)
            .display(Display::Flex)
            .direction(direction)
            .background(Color::WHITE),
    )
}

fn boxed(id: u64, w: f32, h: f32, color: Color) -> Node {
    Node::container(NodeId(id)).with_style(
        Style::default()
            .size(Length::Px(w), Length::Px(h))
            .background(color),
    )
}

fn engine(scene: &Scene) -> Engine {
    Engine::with_fonts(OWNER, scene, &[&font()], W, H).expect("native import")
}

fn render(scene: &Scene) -> Vec<u8> {
    let mut engine = engine(scene);
    let mut bytes = vec![0u8; (W * H * 4) as usize];
    assert!(engine.render_to_surface(
        OWNER,
        scene.version(),
        &mut bytes,
        (W * 4) as usize,
        1 << 24
    ));
    bytes
}

fn px(frame: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    frame[i..i + 4].try_into().unwrap()
}

fn near(a: [u8; 4], b: [u8; 4], tolerance: i32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= tolerance)
}

const BLUE: [u8; 4] = [30, 110, 220, 255];
const RED: [u8; 4] = [220, 50, 47, 255];

// ---------------------------------------------------------------- E2

#[test]
fn e2_rounded_corner_lets_background_through() {
    let rounded = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(60.0), Length::Px(60.0))
            .background(Color::BLUE)
            .corner_radius(20.0),
    )));
    let frame = render(&rounded);
    assert_eq!(px(&frame, 20, 20), BG, "corner tip shows the background");
    assert_eq!(px(&frame, 79, 79), BG, "bottom-right tip");
    assert_eq!(px(&frame, 50, 50), BLUE, "centre");
    assert_eq!(px(&frame, 50, 20), BLUE, "top edge middle is not cut");
    // Control: the same box without radius covers its corner.
    let square = Scene::new(root(FlexDirection::Column).child(boxed(2, 60.0, 60.0, Color::BLUE)));
    assert_eq!(px(&render(&square), 20, 20), BLUE);

    // Per-corner radii: only the rounded corners are cut.
    let mixed = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(60.0), Length::Px(60.0))
            .background(Color::BLUE)
            .corner_radii(CornerRadii::new(20.0, 0.0, 0.0, 0.0)),
    )));
    let frame = render(&mixed);
    assert_eq!(px(&frame, 20, 20), BG);
    assert_eq!(px(&frame, 79, 20), BLUE);
    assert_eq!(px(&frame, 79, 79), BLUE);
    assert_eq!(px(&frame, 20, 79), BLUE);
}

#[test]
fn e2_overflow_clip_follows_the_radius() {
    let clipped = Scene::new(
        root(FlexDirection::Column).child(
            Node::container(NodeId(2))
                .with_style(
                    Style::default()
                        .size(Length::Px(60.0), Length::Px(60.0))
                        .corner_radius(20.0)
                        .overflow(Overflow::Hidden),
                )
                .child(boxed(3, 60.0, 60.0, Color::RED)),
        ),
    );
    let frame = render(&clipped);
    assert_eq!(px(&frame, 20, 20), BG, "child clipped by the rounded parent");
    assert_eq!(px(&frame, 50, 50), RED);
}

// ---------------------------------------------------------------- E3

#[test]
fn e3_shadow_paints_outside_the_box_without_layout() {
    let plain = Scene::new(root(FlexDirection::Column).child(boxed(2, 40.0, 40.0, Color::BLUE)));
    let spread = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(40.0), Length::Px(40.0))
            .background(Color::BLUE)
            .shadow(BoxShadow::new(0.0, 0.0, 0.0, 6.0, Color::BLACK)),
    )));
    let frame = render(&spread);
    // Box spans x 20..60; a 6px spread reaches x 14.
    assert_eq!(px(&frame, 16, 40), [0, 0, 0, 255], "inside the spread");
    assert_eq!(px(&frame, 10, 40), BG, "beyond the spread");
    assert_eq!(px(&frame, 40, 40), BLUE, "the box covers its own shadow");
    // Layout and the plain area are unchanged.
    assert_eq!(
        engine(&plain).node_bounds(OWNER, plain.version(), NodeId(2)),
        engine(&spread).node_bounds(OWNER, spread.version(), NodeId(2))
    );

    // Offset + blur, two layers: both darken their own side.
    let layered = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(40.0), Length::Px(40.0))
            .background(Color::BLUE)
            .shadow(BoxShadow::new(10.0, 10.0, 4.0, 0.0, Color::BLACK))
            .shadow(BoxShadow::new(-8.0, 0.0, 0.0, 0.0, Color::RED)),
    )));
    let frame = render(&layered);
    let below_right = px(&frame, 65, 65);
    assert!(below_right[0] < 200, "blurred offset shadow darkens {below_right:?}");
    assert_eq!(px(&frame, 15, 40), RED, "second layer offset to the left");

    // Inset shadow darkens inside the box edge.
    let inset = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(40.0), Length::Px(40.0))
            .background(Color::WHITE)
            .shadow(BoxShadow {
                inset: true,
                ..BoxShadow::new(0.0, 0.0, 0.0, 5.0, Color::BLACK)
            }),
    )));
    let frame = render(&inset);
    assert_eq!(px(&frame, 22, 40), [0, 0, 0, 255]);
    assert_eq!(px(&frame, 40, 40), BG);
}

// ---------------------------------------------------------------- E4

#[test]
fn e4_opacity_blends_the_whole_subtree() {
    let half = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(40.0), Length::Px(40.0))
            .background(Color::rgb(0, 0, 255))
            .opacity(0.5),
    )));
    let p = px(&render(&half), 40, 40);
    assert!(near(p, [127, 127, 255, 255], 2), "50% blue over white: {p:?}");

    // Group opacity: an opaque child inside a 50% parent is blended once, as one layer.
    let group = Scene::new(
        root(FlexDirection::Column).child(
            Node::container(NodeId(2))
                .with_style(
                    Style::default()
                        .size(Length::Px(40.0), Length::Px(40.0))
                        .background(Color::rgb(0, 0, 255))
                        .opacity(0.5),
                )
                .child(boxed(3, 20.0, 20.0, Color::rgb(255, 0, 0))),
        ),
    );
    let frame = render(&group);
    let child = px(&frame, 25, 25);
    assert!(near(child, [255, 127, 127, 255], 2), "child: {child:?}");
    let parent = px(&frame, 50, 50);
    assert!(near(parent, [127, 127, 255, 255], 2), "parent: {parent:?}");

    let invisible = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(40.0), Length::Px(40.0))
            .background(Color::BLUE)
            .opacity(0.0),
    )));
    assert_eq!(px(&render(&invisible), 40, 40), BG);
}

// ---------------------------------------------------------------- E9

#[test]
fn e9_single_side_border_only_on_that_side() {
    let bottom = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(60.0), Length::Px(40.0))
            .background(Color::WHITE)
            .border_side(
                Side::Bottom,
                Some(BorderSide {
                    width: 4.0,
                    color: Color::RED,
                }),
            ),
    )));
    let frame = render(&bottom);
    // Box y 20..60: bottom border rows 56..60.
    assert_eq!(px(&frame, 50, 58), RED);
    assert_eq!(px(&frame, 50, 21), BG, "no top border");
    assert_eq!(px(&frame, 21, 40), BG, "no left border");
    assert_eq!(px(&frame, 78, 40), BG, "no right border");

    // Uniform border with one side overridden.
    let mixed = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(60.0), Length::Px(40.0))
            .background(Color::WHITE)
            .border(2.0, Color::GREEN)
            .border_side(
                Side::Top,
                Some(BorderSide {
                    width: 6.0,
                    color: Color::BLUE,
                }),
            ),
    )));
    let frame = render(&mixed);
    assert_eq!(px(&frame, 50, 24), BLUE, "top is 6px blue");
    assert_eq!(px(&frame, 50, 27), BG);
    assert_eq!(px(&frame, 20, 40), [46, 160, 67, 255], "left keeps the uniform green");
    assert_eq!(px(&frame, 23, 40), BG);
}

#[test]
fn e9_outline_is_outside_the_box_and_takes_no_layout() {
    let style = Style::default()
        .size(Length::Px(40.0), Length::Px(40.0))
        .background(Color::BLUE);
    let plain = Scene::new(
        root(FlexDirection::Row)
            .child(Node::container(NodeId(2)).with_style(style.clone()))
            .child(boxed(3, 20.0, 20.0, Color::GREEN)),
    );
    let ringed = Scene::new(
        root(FlexDirection::Row)
            .child(Node::container(NodeId(2)).with_style(style.clone().outline(Some(Outline {
                width: 2.0,
                offset: 3.0,
                color: Color::RED,
            }))))
            .child(boxed(3, 20.0, 20.0, Color::GREEN)),
    );
    let frame = render(&ringed);
    // Box x 20..60, y 20..60. Ring spans 3..5 px outside: x 15..17 and 63..65.
    assert_eq!(px(&frame, 16, 40), RED, "left ring");
    assert_eq!(px(&frame, 63, 40), RED, "right ring");
    assert_eq!(px(&frame, 40, 16), RED, "top ring");
    assert_eq!(px(&frame, 18, 40), BG, "offset gap");
    assert_eq!(px(&frame, 12, 40), BG, "beyond the ring");
    assert_eq!(px(&frame, 40, 40), BLUE, "inside untouched");
    let a = engine(&plain);
    let b = engine(&ringed);
    for id in [2, 3] {
        assert_eq!(
            a.node_bounds(OWNER, plain.version(), NodeId(id)),
            b.node_bounds(OWNER, ringed.version(), NodeId(id)),
            "layout of node {id}"
        );
    }

    // Own overflow does not clip the ring; a clipping ancestor does.
    let own_clip = Scene::new(root(FlexDirection::Row).child(Node::container(NodeId(2)).with_style(
        style.clone().overflow(Overflow::Hidden).outline(Some(Outline {
            width: 2.0,
            offset: 3.0,
            color: Color::RED,
        })),
    )));
    assert_eq!(px(&render(&own_clip), 16, 40), RED);
    let ancestor_clip = Scene::new(
        root(FlexDirection::Row).child(
            Node::container(NodeId(4))
                .with_style(
                    Style::default()
                        .size(Length::Px(40.0), Length::Px(40.0))
                        .overflow(Overflow::Hidden),
                )
                .child(Node::container(NodeId(2)).with_style(style.clone().outline(Some(
                    Outline {
                        width: 2.0,
                        offset: 3.0,
                        color: Color::RED,
                    },
                )))),
        ),
    );
    assert_eq!(px(&render(&ancestor_clip), 16, 40), BG);

    // Negative offset: an inset ring inside the box.
    let inset = Scene::new(root(FlexDirection::Row).child(Node::container(NodeId(2)).with_style(
        style.clone().outline(Some(Outline {
            width: 2.0,
            offset: -4.0,
            color: Color::RED,
        })),
    )));
    let frame = render(&inset);
    assert_eq!(px(&frame, 23, 40), RED);
    assert_eq!(px(&frame, 21, 40), BLUE);
    assert_eq!(px(&frame, 26, 40), BLUE);

    // Rounded ring follows the radius: the outer corner tip stays background.
    let rounded = Scene::new(root(FlexDirection::Row).child(Node::container(NodeId(2)).with_style(
        style.clone().corner_radius(10.0).outline(Some(Outline {
            width: 2.0,
            offset: 2.0,
            color: Color::RED,
        })),
    )));
    let frame = render(&rounded);
    assert_eq!(px(&frame, 16, 16), BG, "outer corner tip is rounded");
    assert_eq!(px(&frame, 16, 40), RED);

    // Ancestor opacity applies to the ring.
    let faded = Scene::new(
        root(FlexDirection::Row).child(
            Node::container(NodeId(4))
                .with_style(Style::default().opacity(0.5))
                .child(Node::container(NodeId(2)).with_style(style.outline(Some(Outline {
                    width: 2.0,
                    offset: 3.0,
                    color: Color::rgb(255, 0, 0),
                })))),
        ),
    );
    let p = px(&render(&faded), 16, 40);
    assert!(near(p, [255, 127, 127, 255], 2), "faded ring {p:?}");
}

/// Rows (y) in 0..H where at least 80% of x in `xs` are dark.
fn dark_rows(frame: &[u8], xs: std::ops::Range<u32>) -> Vec<u32> {
    (0..H)
        .filter(|&y| {
            let dark = xs.clone().filter(|&x| px(frame, x, y)[0] < 128).count();
            dark * 5 >= xs.len() * 4
        })
        .collect()
}

#[test]
fn e9_text_decoration_draws_lines() {
    let text = |decoration: TextDecoration| {
        Scene::new(
            root(FlexDirection::Column).child(
                Node::text(NodeId(2), "xxxxxxxx").with_style(
                    Style::default()
                        .text(20.0, Color::BLACK)
                        .text_decoration(decoration),
                ),
            ),
        )
    };
    let plain = render(&text(TextDecoration::NONE));
    let underlined = render(&text(TextDecoration::UNDERLINE));
    let struck = render(&text(TextDecoration::LINE_THROUGH));
    let span = 22..90;
    assert!(dark_rows(&plain, span.clone()).is_empty(), "glyph gaps keep plain rows light");
    let under = dark_rows(&underlined, span.clone());
    let through = dark_rows(&struck, span.clone());
    assert!(!under.is_empty(), "underline row exists");
    assert!(!through.is_empty(), "line-through row exists");
    assert!(
        under.iter().min() > through.iter().max(),
        "underline {under:?} below line-through {through:?}"
    );
}

// ---------------------------------------------------------------- E10 and E2 hit testing

fn button(id: u64, w: f32, h: f32, style: Style) -> Node {
    Node::container(NodeId(id))
        .with_style(style.size(Length::Px(w), Length::Px(h)).background(Color::BLUE))
        .with_semantics(
            Semantics::default()
                .role(Role::Button)
                .label(format!("b{id}"))
                .focusable(true),
        )
}

/// Press and release at one point; returns (activated nodes, Down event (node, position)).
fn tap(engine: &mut Engine, v: u64, x: f32, y: f32) -> (Vec<u64>, Option<(u64, (f32, f32))>) {
    let mut activated = Vec::new();
    let mut down = None;
    for (phase, pressed) in [
        (PointerAction::Move, false),
        (PointerAction::Down, true),
        (PointerAction::Up, false),
    ] {
        assert!(engine.pointer(
            OWNER,
            v,
            phase,
            x,
            y,
            pressed,
            &mut |e| {
                match e.kind {
                    SceneEventKind::Activate => activated.push(e.node.0),
                    SceneEventKind::Pointer(PointerAction::Down) if down.is_none() => {
                        down = Some((e.node.0, e.position))
                    }
                    _ => {}
                }
                EventReply::default()
            },
            &mut |_| {},
        ));
    }
    (activated, down)
}

#[test]
fn e10_hit_slop_extends_hits_without_layout_or_paint() {
    let slop = Style::default().hit_slop(Edges::all(8.0));
    let with = Scene::new(root(FlexDirection::Column).child(button(2, 40.0, 20.0, slop)));
    let without =
        Scene::new(root(FlexDirection::Column).child(button(2, 40.0, 20.0, Style::default())));
    // Button x 20..60, y 20..40. (15, 30) is 5px left of it.
    let mut a = engine(&with);
    let mut b = engine(&without);
    assert_eq!(
        a.node_bounds(OWNER, with.version(), NodeId(2)),
        b.node_bounds(OWNER, without.version(), NodeId(2))
    );
    assert_eq!(render(&with), render(&without), "slop is not painted");
    let (activated, down) = tap(&mut a, with.version(), 15.0, 30.0);
    assert_eq!(activated, vec![2]);
    assert_eq!(down, Some((2, (15.0, 30.0))), "actual position is reported");
    let (activated, down) = tap(&mut b, without.version(), 15.0, 30.0);
    assert!(activated.is_empty());
    assert_eq!(down.map(|d| d.0), Some(1));
    // Beyond the slop: nothing.
    assert!(tap(&mut a, with.version(), 10.0, 30.0).0.is_empty());
    // Accessibility is unchanged by the slop.
    assert_eq!(
        a.semantics(OWNER, with.version()).unwrap().nodes,
        b.semantics(OWNER, without.version()).unwrap().nodes
    );
}

#[test]
fn e10_real_box_beats_a_neighbours_slop_and_nearest_slop_wins() {
    // Row: button 2 at x 20..60, gap 10, button 3 at x 70..110; both with 8px slop.
    let slop = || Style::default().hit_slop(Edges::all(8.0));
    let scene = Scene::new(
        root(FlexDirection::Row)
            .child(button(2, 40.0, 20.0, slop()))
            .child(button(3, 40.0, 20.0, slop())),
    );
    let v = scene.version();
    let mut e = engine(&scene);
    assert_eq!(tap(&mut e, v, 71.0, 30.0).0, vec![3], "inside 3, within 2's slop");
    assert_eq!(tap(&mut e, v, 62.0, 30.0).0, vec![2], "gap, nearer 2");
    assert_eq!(tap(&mut e, v, 68.0, 30.0).0, vec![3], "gap, nearer 3");
}

#[test]
fn e2_rounded_corner_is_not_hit() {
    let scene = Scene::new(
        root(FlexDirection::Column)
            .child(button(2, 60.0, 60.0, Style::default().corner_radius(30.0))),
    );
    let v = scene.version();
    let mut e = engine(&scene);
    let (activated, down) = tap(&mut e, v, 21.0, 21.0);
    assert!(activated.is_empty(), "corner of a circle");
    assert_eq!(down.map(|d| d.0), Some(1), "the root receives the press");
    assert_eq!(tap(&mut e, v, 50.0, 50.0).0, vec![2]);
    assert_eq!(tap(&mut e, v, 50.0, 21.0).0, vec![2], "top edge middle");
}

// ---------------------------------------------------------------- contract

#[test]
fn invalid_extended_style_is_rejected_whole() {
    let source = Scene::new(root(FlexDirection::Column).child(boxed(2, 40.0, 40.0, Color::BLUE)));
    let v = source.version();
    let mut e = engine(&source);
    for bad in [
        Style::default().opacity(1.5),
        Style::default().corner_radius(-1.0),
        Style::default().hit_slop(Edges::all(f32::NAN)),
    ] {
        let patches = [ScenePatch::UpdateStyle {
            id: NodeId(2),
            style: bad,
        }];
        assert_eq!(
            e.try_apply_update(
                OWNER,
                SceneUpdate {
                    expected_version: v,
                    next_version: v + 1,
                    patches: &patches,
                }
            ),
            Err(SceneUpdateError::NativeRejected)
        );
    }
    let invalid = Scene::new(root(FlexDirection::Column).child(Node::container(NodeId(2)).with_style(
        Style::default().opacity(-0.5),
    )));
    assert!(Engine::with_fonts(OWNER, &invalid, &[&font()], W, H).is_none());

    // A valid update changes pixels, and clearing it restores the original frame.
    let original = render(&source);
    let mut scene = source.clone();
    let patches = [ScenePatch::UpdateStyle {
        id: NodeId(2),
        style: Style::default()
            .size(Length::Px(40.0), Length::Px(40.0))
            .background(Color::BLUE)
            .corner_radius(20.0),
    }];
    scene.apply_batch_at(v, &patches).unwrap();
    assert!(e.apply_update(
        OWNER,
        SceneUpdate {
            expected_version: v,
            next_version: scene.version(),
            patches: &patches,
        }
    ));
    let mut frame = vec![0u8; (W * H * 4) as usize];
    assert!(e.render_to_surface(OWNER, scene.version(), &mut frame, (W * 4) as usize, 1 << 24));
    assert_eq!(px(&frame, 20, 20), BG);
    assert_ne!(frame, original);
}

#[test]
fn capabilities_are_declared_for_the_host_hello() {
    let names: Vec<&str> = style_capabilities().iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        [
            "corner-radius",
            "shadow",
            "opacity",
            "border-side",
            "focus-ring",
            "underline",
            "hit-target",
            "transform",
            "transition"
        ]
    );
}


#[test]
fn e9_text_decoration_propagates_from_a_container_in_its_colour() {
    // A centred link container decorates its caption child, in the container's text colour.
    let link = |decoration: TextDecoration| {
        Scene::new(
            root(FlexDirection::Column).child(
                Node::container(NodeId(2))
                    .with_style(
                        Style::default()
                            .size(Length::Px(120.0), Length::Px(40.0))
                            .align_items(jasper_ui_engine::style::Align::Center)
                            .text(20.0, Color::rgb(0, 0, 255))
                            .text_decoration(decoration),
                    )
                    .with_semantics(Semantics::default().role(Role::Link).label("link"))
                    .child(
                        Node::text(NodeId(3), "xxxxxx")
                            .with_style(Style::default().text(20.0, Color::BLACK)),
                    ),
            ),
        )
    };
    let plain = render(&link(TextDecoration::NONE));
    let underlined = render(&link(TextDecoration::UNDERLINE));
    let text = engine(&link(TextDecoration::NONE))
        .node_bounds(OWNER, link(TextDecoration::NONE).version(), NodeId(3))
        .unwrap();
    let (x0, x1) = (
        text.origin.x as u32 + 3,
        (text.origin.x + text.size.width) as u32 - 3,
    );
    assert!(x0 > 25, "caption is centred, not at the left edge");
    assert!(dark_rows(&plain, x0..x1).is_empty());
    let rows: Vec<u32> = (0..H)
        .filter(|&y| {
            (x0..x1).all(|x| {
                let [r, g, b, _] = px(&underlined, x, y);
                r < 110 && g < 110 && b > 200
            })
        })
        .collect();
    assert!(!rows.is_empty(), "a solid blue underline under the caption");
    // Layout and accessibility do not change with the decoration.
    let a = link(TextDecoration::NONE);
    let b = link(TextDecoration::UNDERLINE);
    assert_eq!(
        engine(&a).semantics(OWNER, a.version()).unwrap().nodes,
        engine(&b).semantics(OWNER, b.version()).unwrap().nodes
    );
    assert_eq!(
        engine(&a).node_bounds(OWNER, a.version(), NodeId(3)),
        engine(&b).node_bounds(OWNER, b.version(), NodeId(3))
    );
}
