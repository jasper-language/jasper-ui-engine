//! E7 transitions and E8 transforms on the native CPU path: frame-by-frame headless pixels on a
//! simulated host clock, retargeting continuity, reduce-motion, springs, transformed hit testing
//! and the unchanged default.
//! 原生 CPU 路径上的 E7 过渡与 E8 变换：按模拟宿主时钟逐帧无头出图、续接连续性、减少动态、弹簧、变换后的命中测试，
//! 以及默认行为不变。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::{Edges, Rect};
use jasper_ui_engine::native::host::{style_capabilities, AnimationTick};
use jasper_ui_engine::native::{EventReply, PointerAction, SceneEventKind};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{
    AnimatedProperties, Color, Display, Easing, FlexDirection, Length, Outline, Style, Transform2D,
    Transition,
};

const OWNER: u64 = 11;
const W: u32 = 160;
const H: u32 = 120;
const BG: [u8; 4] = [255, 255, 255, 255];
const BLUE: [u8; 4] = [30, 110, 220, 255];
const RED: [u8; 4] = [220, 50, 47, 255];

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT")).unwrap()
}

/// White W×H root with 20px padding; the child flows from (20, 20).
fn root() -> Node {
    Node::container(NodeId(1)).with_style(
        Style::default()
            .size(Length::Px(W as f32), Length::Px(H as f32))
            .padding(Edges::all(20.0))
            .gap(10.0)
            .display(Display::Flex)
            .direction(FlexDirection::Column)
            .background(Color::WHITE),
    )
}

/// 40×40 blue button-like box (id 2) with `style` on top.
fn boxed(style: Style) -> Node {
    Node::container(NodeId(2))
        .with_style(style.size(Length::Px(40.0), Length::Px(40.0)))
        .with_semantics(
            Semantics::default()
                .role(Role::Button)
                .label("b")
                .focusable(true),
        )
}

fn base() -> Style {
    Style::default().background(Color::BLUE)
}

fn scene(style: Style) -> Scene {
    Scene::new(root().child(boxed(style)))
}

struct View {
    engine: Engine,
    version: u64,
}

impl View {
    fn new(style: Style) -> Self {
        let scene = scene(style);
        let version = scene.version();
        Self {
            engine: Engine::with_fonts(OWNER, &scene, &[&font()], W, H).expect("native import"),
            version,
        }
    }
    fn restyle(&mut self, style: Style) {
        let style = style.size(Length::Px(40.0), Length::Px(40.0));
        self.engine
            .try_apply(
                OWNER,
                self.version,
                self.version + 1,
                &[ScenePatch::UpdateStyle {
                    id: NodeId(2),
                    style,
                }],
            )
            .expect("update");
        self.version += 1;
    }
    fn animate(&mut self, now: f64) -> AnimationTick {
        self.engine.animate(OWNER, now).expect("animate")
    }
    fn frame(&mut self) -> Vec<u8> {
        let mut bytes = vec![0u8; (W * H * 4) as usize];
        assert!(self.engine.render_to_surface(
            OWNER,
            self.version,
            &mut bytes,
            (W * 4) as usize,
            1 << 24
        ));
        bytes
    }
    /// Node under a primary press at (x, y), from the Down event.
    fn hit(&mut self, x: f32, y: f32) -> Option<u64> {
        let mut down = None;
        for (phase, pressed) in [
            (PointerAction::Move, false),
            (PointerAction::Down, true),
            (PointerAction::Up, false),
        ] {
            assert!(self.engine.pointer(
                OWNER,
                self.version,
                phase,
                x,
                y,
                pressed,
                &mut |e| {
                    if let SceneEventKind::Pointer(PointerAction::Down) = e.kind {
                        down.get_or_insert(e.node.0);
                    }
                    EventReply::default()
                },
                &mut |_| {},
            ));
        }
        down
    }
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

fn mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let mut out = [0u8; 4];
    for i in 0..4 {
        out[i] = (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    }
    out
}

fn linear(properties: AnimatedProperties, ms: f32) -> Transition {
    Transition::new(properties, ms, Easing::LINEAR)
}

#[test]
fn capabilities_declare_transform_and_transition() {
    let names: Vec<&str> = style_capabilities().iter().map(|c| c.name).collect();
    assert!(names.contains(&"transform") && names.contains(&"transition"));
    let keys = |name: &str| {
        style_capabilities()
            .iter()
            .find(|c| c.name == name)
            .unwrap()
            .xstyle_keys
    };
    assert_eq!(keys("transform"), &["transform"]);
    assert_eq!(keys("transition"), &["transition"]);
}

#[test]
fn default_without_transition_is_immediate_and_identical() {
    // Clock attached, no transition: the update shows in the very next frame, nothing runs, and
    // the pixels equal a fresh view of the final scene.
    let mut view = View::new(base());
    view.animate(0.0);
    view.frame();
    view.restyle(base().background(Color::RED).opacity(0.5));
    assert_eq!(view.engine.animating(OWNER), Some(false));
    let tick = view.animate(16.0);
    assert_eq!(tick, AnimationTick::default());
    let frame = view.frame();
    assert_eq!(view.engine.needs_frame(OWNER), Some(false));
    let mut fresh = View::new(base().background(Color::RED).opacity(0.5));
    assert_eq!(frame, fresh.frame());
    // An identity transform (any origin) paints exactly like no transform.
    let plain = View::new(base()).frame();
    let identity = View::new(base().transform(Transform2D::IDENTITY.with_origin(0.0, 1.0))).frame();
    assert_eq!(plain, identity);
}

#[test]
fn without_a_host_clock_transitions_are_immediate() {
    let mut view = View::new(base().transition(linear(AnimatedProperties::ALL, 500.0)));
    view.restyle(
        base()
            .background(Color::RED)
            .transition(linear(AnimatedProperties::ALL, 500.0)),
    );
    assert_eq!(view.engine.animating(OWNER), Some(false));
    assert_eq!(px(&view.frame(), 40, 40), RED);
}

#[test]
fn opacity_and_colour_interpolate_frame_by_frame() {
    let rule = linear(
        AnimatedProperties::OPACITY.union(AnimatedProperties::BACKGROUND),
        200.0,
    );
    let mut view = View::new(base().transition(rule));
    view.animate(1000.0);
    assert_eq!(px(&view.frame(), 40, 40), BLUE);
    let version = view.version;
    view.restyle(base().opacity(0.0).transition(rule));
    assert_eq!(view.engine.animating(OWNER), Some(true));
    assert_eq!(view.engine.needs_frame(OWNER), Some(true));
    // First sample starts the channel: still the old value.
    view.animate(1000.0);
    assert_eq!(px(&view.frame(), 40, 40), BLUE);
    for (t, expected) in [(1050.0, 0.25), (1100.0, 0.5), (1150.0, 0.75)] {
        let tick = view.animate(t);
        assert!(tick.changed && tick.running);
        let p = px(&view.frame(), 40, 40);
        let want = mix(BLUE, BG, expected);
        assert!(near(p, want, 3), "t={t}: {p:?} vs {want:?}");
        // Animation never consumes the scene version.
        assert_eq!(view.version, version + 1);
        assert!(view
            .engine
            .node_bounds(OWNER, view.version, NodeId(2))
            .is_some());
    }
    let tick = view.animate(1200.0);
    assert!(tick.changed && !tick.running);
    assert_eq!(px(&view.frame(), 40, 40), BG);
    assert_eq!(view.engine.needs_frame(OWNER), Some(false));
    assert_eq!(view.animate(1300.0), AnimationTick::default());

    // Background colour: blue → red, middle value at half time.
    let mut view = View::new(base().transition(rule));
    view.animate(0.0);
    view.restyle(base().background(Color::RED).transition(rule));
    view.animate(0.0);
    view.animate(100.0);
    let p = px(&view.frame(), 40, 40);
    assert!(near(p, mix(BLUE, RED, 0.5), 2), "{p:?}");
    view.animate(200.0);
    assert_eq!(px(&view.frame(), 40, 40), RED);
}

#[test]
fn retarget_continues_from_the_presented_value() {
    let rule = linear(AnimatedProperties::OPACITY, 200.0);
    let mut view = View::new(base().transition(rule));
    view.animate(0.0);
    view.restyle(base().opacity(0.0).transition(rule));
    view.animate(0.0);
    view.animate(100.0);
    let half = px(&view.frame(), 40, 40);
    assert!(near(half, mix(BLUE, BG, 0.5), 3));
    // Back to opaque mid-way: the next frame shows the same pixel (no jump), then fades in.
    view.restyle(base().transition(rule));
    assert_eq!(px(&view.frame(), 40, 40), half, "no jump at retarget");
    view.animate(100.0);
    assert_eq!(px(&view.frame(), 40, 40), half);
    view.animate(200.0);
    let p = px(&view.frame(), 40, 40);
    assert!(near(p, mix(BLUE, BG, 0.25), 3), "{p:?}");
    view.animate(300.0);
    assert_eq!(px(&view.frame(), 40, 40), BLUE);
    assert_eq!(view.engine.animating(OWNER), Some(false));
}

/// Leftmost blue column on row `y`.
fn left_edge(frame: &[u8], y: u32) -> Option<u32> {
    (0..W).find(|x| near(px(frame, *x, y), BLUE, 40))
}

#[test]
fn transform_transition_moves_paint_and_hit_testing_not_layout() {
    let rule = linear(AnimatedProperties::TRANSFORM, 100.0);
    let mut view = View::new(base().transition(rule));
    let layout = view
        .engine
        .node_bounds(OWNER, view.version, NodeId(2))
        .unwrap();
    assert_eq!(layout, Rect::new(20.0, 20.0, 40.0, 40.0));
    view.animate(0.0);
    view.restyle(
        base()
            .transform(Transform2D::translate(60.0, 0.0))
            .transition(rule),
    );
    view.animate(0.0);
    assert_eq!(left_edge(&view.frame(), 40), Some(20));
    view.animate(50.0);
    let frame = view.frame();
    assert_eq!(left_edge(&frame, 40), Some(50), "half way");
    // Hit testing follows the presented position; layout bounds do not move.
    assert_eq!(view.hit(55.0, 40.0), Some(2));
    assert_eq!(view.hit(25.0, 40.0), Some(1));
    assert_eq!(
        view.engine.node_bounds(OWNER, view.version, NodeId(2)),
        Some(layout)
    );
    view.animate(100.0);
    let frame = view.frame();
    assert_eq!(left_edge(&frame, 40), Some(80));
    assert_eq!(px(&frame, 40, 40), BG, "old place is empty");
    assert_eq!(view.hit(100.0, 40.0), Some(2));
    assert_eq!(view.hit(40.0, 40.0), Some(1));
    assert_eq!(view.engine.animating(OWNER), Some(false));
}

#[test]
fn reduce_motion_makes_movement_immediate_and_uses_declared_cross_fade() {
    let rule = linear(
        AnimatedProperties::TRANSFORM.union(AnimatedProperties::OPACITY),
        200.0,
    )
    .reduced(100.0, Easing::LINEAR);
    let mut view = View::new(base().transition(rule));
    view.animate(0.0);
    assert!(view.engine.set_reduce_motion(OWNER, true));
    view.restyle(
        base()
            .transform(Transform2D::translate(60.0, 0.0))
            .opacity(0.0)
            .transition(rule),
    );
    view.animate(0.0);
    view.animate(16.0);
    let frame = view.frame();
    // Movement lands in one frame; opacity cross-fades over the declared 100 ms.
    let p = px(&frame, 100, 40);
    assert!(near(p, mix(BLUE, BG, 0.16), 4), "{p:?}");
    assert_eq!(px(&frame, 40, 40), BG);
    view.animate(100.0);
    assert_eq!(px(&view.frame(), 100, 40), BG);

    // Without a declared alternative every property is immediate under reduce-motion.
    let plain = linear(AnimatedProperties::ALL, 200.0);
    let mut view = View::new(base().transition(plain));
    view.animate(0.0);
    view.engine.set_reduce_motion(OWNER, true);
    view.restyle(
        base()
            .transform(Transform2D::translate(60.0, 0.0))
            .background(Color::RED)
            .transition(plain),
    );
    assert_eq!(view.engine.animating(OWNER), Some(false));
    let frame = view.frame();
    assert_eq!(px(&frame, 100, 40), RED);
    assert_eq!(px(&frame, 40, 40), BG);

    // Turning reduce-motion on mid-animation lands at once.
    let mut view = View::new(base().transition(plain));
    view.animate(0.0);
    view.restyle(base().background(Color::RED).transition(plain));
    view.animate(0.0);
    view.animate(50.0);
    assert_ne!(px(&view.frame(), 40, 40), RED);
    view.engine.set_reduce_motion(OWNER, true);
    assert_eq!(view.engine.animating(OWNER), Some(false));
    assert_eq!(px(&view.frame(), 40, 40), RED);
}

#[test]
fn spring_overshoots_then_settles_with_finite_frames() {
    let rule = Transition::spring(AnimatedProperties::TRANSFORM, 1.0, 170.0, 12.0);
    let mut view = View::new(base().transition(rule));
    view.animate(0.0);
    view.restyle(
        base()
            .transform(Transform2D::translate(40.0, 0.0))
            .transition(rule),
    );
    view.animate(0.0);
    let mut t = 0.0;
    let mut furthest = 0;
    let mut frames = 0;
    while view.engine.animating(OWNER) == Some(true) {
        t += 16.0;
        frames += 1;
        assert!(frames < 400, "spring must settle");
        view.animate(t);
        if frames % 3 == 0 {
            if let Some(edge) = left_edge(&view.frame(), 40) {
                furthest = furthest.max(edge);
            }
        }
    }
    assert!(furthest > 60, "under-damped spring overshoots: {furthest}");
    assert_eq!(
        left_edge(&view.frame(), 40),
        Some(60),
        "settles exactly on target"
    );
}

#[test]
fn static_transforms_paint_and_hit_about_the_origin() {
    // 40×40 box at (20,20) scaled ×2 about its centre covers (0,0)–(80,80).
    let mut view = View::new(base().transform(Transform2D::scale(2.0)));
    let frame = view.frame();
    assert_eq!(px(&frame, 5, 5), BLUE);
    assert_eq!(px(&frame, 75, 75), BLUE);
    assert_eq!(px(&frame, 85, 85), BG);
    assert_eq!(
        view.hit(5.0, 5.0),
        Some(2),
        "outside the layout box, inside the paint"
    );
    assert_eq!(view.hit(85.0, 85.0), Some(1));
    assert_eq!(
        view.engine.node_bounds(OWNER, view.version, NodeId(2)),
        Some(Rect::new(20.0, 20.0, 40.0, 40.0)),
        "layout and node_bounds ignore the transform"
    );
    // Scaled down about the top-left origin: the far part of the layout box is empty and
    // not hit.
    let mut view = View::new(base().transform(Transform2D::scale(0.5).with_origin(0.0, 0.0)));
    let frame = view.frame();
    assert_eq!(px(&frame, 25, 25), BLUE);
    assert_eq!(px(&frame, 50, 50), BG);
    assert_eq!(view.hit(50.0, 50.0), Some(1));
    assert_eq!(view.hit(25.0, 25.0), Some(2));
    // Rotation by 45° about the centre: the layout corner is outside, a diagonal tip inside.
    let mut view = View::new(base().transform(Transform2D::rotate(45.0)));
    let frame = view.frame();
    assert_eq!(px(&frame, 22, 22), BG);
    assert_eq!(px(&frame, 40, 14), BLUE, "top tip beyond the layout box");
    assert_eq!(view.hit(22.0, 22.0), Some(1));
    assert_eq!(view.hit(40.0, 14.0), Some(2));
}

#[test]
fn transform_does_not_move_siblings() {
    let sibling = Node::container(NodeId(3)).with_style(
        Style::default()
            .size(Length::Px(40.0), Length::Px(20.0))
            .background(Color::RED),
    );
    let plain = Scene::new(root().child(boxed(base())).child(sibling.clone()));
    let moved = Scene::new(
        root()
            .child(boxed(base().transform(Transform2D::translate(80.0, 0.0))))
            .child(sibling),
    );
    for scene in [&plain, &moved] {
        let engine = Engine::with_fonts(OWNER, scene, &[&font()], W, H).unwrap();
        assert_eq!(
            engine.node_bounds(OWNER, scene.version(), NodeId(3)),
            Some(Rect::new(20.0, 70.0, 40.0, 20.0))
        );
    }
}

#[test]
fn engine_overlays_and_corner_cuts_follow_the_transform() {
    // Outline (engine-painted) moves with a translated node.
    let ring = Outline {
        width: 2.0,
        offset: 2.0,
        color: Color::rgb(0, 0, 0),
    };
    let mut view = View::new(
        base()
            .outline(Some(ring))
            .transform(Transform2D::translate(60.0, 0.0)),
    );
    let frame = view.frame();
    assert_eq!(
        px(&frame, 77, 40),
        [0, 0, 0, 255],
        "ring left of the moved box"
    );
    assert_eq!(px(&frame, 17, 40), BG, "no ring at the layout place");
    // A rounded, translated node: the moved corner is cut for hit testing.
    let mut view = View::new(
        base()
            .corner_radius(20.0)
            .transform(Transform2D::translate(60.0, 0.0)),
    );
    let frame = view.frame();
    assert_eq!(px(&frame, 81, 21), BG);
    assert_eq!(view.hit(81.0, 21.0), Some(1), "cut corner of the moved box");
    assert_eq!(view.hit(100.0, 40.0), Some(2));
}

#[test]
fn removed_node_drops_its_transition_and_inserted_nodes_do_not_animate() {
    let rule = linear(AnimatedProperties::ALL, 200.0);
    let mut view = View::new(base().transition(rule));
    view.animate(0.0);
    view.restyle(base().background(Color::RED).transition(rule));
    assert_eq!(view.engine.animating(OWNER), Some(true));
    view.engine
        .try_apply(
            OWNER,
            view.version,
            view.version + 1,
            &[
                ScenePatch::Remove { id: NodeId(2) },
                ScenePatch::Insert {
                    parent: NodeId(1),
                    index: 0,
                    node: boxed(base().background(Color::RED).transition(rule)),
                },
            ],
        )
        .unwrap();
    view.version += 1;
    assert_eq!(view.engine.animating(OWNER), Some(false));
    assert_eq!(px(&view.frame(), 40, 40), RED);
}
