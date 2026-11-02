//! Item 5 public caller regression: a visual-only switch must preserve focus, scroll and
//! accessibility, while rejected host resources leave the active view usable.
//! 第 5 项公共调用方回归：纯视觉切换保留焦点、滚动与无障碍，资源拒绝后活动视图仍可用。
#![cfg(feature = "native-blitz-cpu-pending")]

use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::HostRenderer;
use jasper_ui_engine::native::{EventReply, PointerAction, SceneEventKind};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{Color, Length, Style};

const OWNER: u64 = 71;
const W: u32 = 180;
const H: u32 = 100;
const BUDGET: usize = 2 * 1024 * 1024;
const BUTTON: NodeId = NodeId(2);

fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1)).child(
            Node::container(NodeId(3))
                .child(
                    Node::text(BUTTON, "Save")
                        .with_semantics(
                            Semantics::default()
                                .role(Role::Button)
                                .label("Save")
                                .focusable(true),
                        )
                        .with_style(
                            Style::default()
                                .size(Length::Px(120.0), Length::Px(48.0))
                                .background(Color::GREEN),
                        ),
                )
                .child(
                    Node::rect(NodeId(4))
                        .with_style(Style::default().size(Length::Px(120.0), Length::Px(80.0))),
                ),
        ),
    )
}

fn font() -> Vec<u8> {
    let path = std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT must be injected");
    std::fs::read(path).expect("injected font readable")
}

#[test]
fn active_native_examples_use_only_the_formal_engine_entry() {
    for (name, source) in [
        ("hello", include_str!("../examples/hello.rs")),
        ("pointer", include_str!("../examples/pointer.rs")),
        ("window", include_str!("../examples/window.rs")),
        ("gpu_window", include_str!("../examples/gpu_window.rs")),
        ("full_scene", include_str!("../examples/full_scene.rs")),
    ] {
        assert!(
            source.contains("engine::Engine") || source.contains("HostRenderer"),
            "{name} must consume a formal public engine entry"
        );
        for private in [
            "native_adapter",
            "blitz_dom",
            "blitz_paint",
            "blitz_traits",
            "anyrender",
            "LegacyEngine",
            "experiments/",
        ] {
            assert!(
                !source.contains(private),
                "{name} leaks private execution path `{private}`"
            );
        }
    }
}

fn render(engine: &mut Engine, version: u64) -> Vec<u8> {
    let mut pixels = vec![0; (W * H * 4) as usize];
    assert!(engine.render_to_surface(OWNER, version, &mut pixels, (W * 4) as usize, BUDGET));
    pixels
}

#[test]
fn visual_switch_keeps_focus_scroll_accessibility_and_rejects_resource_failures() {
    let font = font();
    let mut source = scene();
    let mut engine = Engine::new(OWNER, &source, &font, W, H).expect("native import");
    let version = source.version();
    assert!(engine.focus(OWNER, version, BUTTON, &mut |_| {}));
    assert!(engine.set_viewport_scroll(OWNER, version, 0.0, 5.0));
    let bounds = engine.node_bounds(OWNER, version, BUTTON).unwrap();
    assert!(bounds.y() < 0.0);
    let tree = engine.accessibility_tree(OWNER, version).unwrap();
    assert_eq!(tree.focused, Some(BUTTON));
    assert!(tree
        .nodes
        .iter()
        .any(|node| node.node == BUTTON && node.actions.activate));

    let mut events = Vec::new();
    assert!(engine.pointer(
        OWNER,
        version,
        PointerAction::Down,
        bounds.x() + 5.0,
        bounds.y() + 5.0,
        true,
        &mut |event| {
            events.push(event.kind);
            EventReply::default()
        },
        &mut |_| {},
    ));
    let patches = [ScenePatch::UpdateStyle {
        id: BUTTON,
        style: Style::default()
            .size(Length::Px(120.0), Length::Px(48.0))
            .background(Color::BLUE),
    }];
    source.apply_batch_at(version, &patches).unwrap();
    assert!(engine.apply_with_focus_events(
        OWNER,
        version,
        source.version(),
        &patches,
        &mut |_| panic!("visual-only update changed focus"),
    ));
    assert_eq!(engine.focused_node(OWNER, source.version()), Some(BUTTON));
    assert_eq!(
        engine.node_bounds(OWNER, source.version(), BUTTON),
        Some(bounds)
    );
    let tree = engine.accessibility_tree(OWNER, source.version()).unwrap();
    assert_eq!(tree.focused, Some(BUTTON));
    assert!(tree
        .nodes
        .iter()
        .any(|node| node.node == BUTTON && node.actions.activate));

    let incremental = render(&mut engine, source.version());
    let mut reference = Engine::new(OWNER, &source, &font, W, H).unwrap();
    assert!(reference.set_viewport_scroll(OWNER, source.version(), 0.0, 5.0));
    assert_eq!(incremental, render(&mut reference, source.version()));

    assert!(engine.pointer(
        OWNER,
        source.version(),
        PointerAction::Up,
        bounds.x() + 5.0,
        bounds.y() + 5.0,
        false,
        &mut |event| {
            events.push(event.kind);
            EventReply::default()
        },
        &mut |_| {},
    ));
    assert_eq!(
        events
            .iter()
            .filter(|kind| **kind == SceneEventKind::Activate)
            .count(),
        1
    );

    let guard = [0xA5; 31];
    let mut surface = guard;
    assert!(!engine.render_to_surface(OWNER, source.version(), &mut surface, (W * 4) as usize, 1));
    assert_eq!(surface, guard);
    assert_eq!(render(&mut engine, source.version()), incremental);

    assert!(engine.close(OWNER));
    assert!(!engine.render_to_surface(
        OWNER,
        source.version(),
        &mut surface,
        (W * 4) as usize,
        BUDGET
    ));
    assert_eq!(surface, guard);
}

#[test]
fn host_renderer_gates_input_and_presentation_by_surface_generation() {
    let font = font();
    let source = scene();
    let mut host =
        HostRenderer::new(OWNER, &source, &font, W, H).expect("native host renderer import");
    let generation = host.surface_status(OWNER).unwrap().generation;
    assert!(host.focus(OWNER, source.version(), generation, BUTTON, &mut |_| {}));
    assert_eq!(
        host.accessibility_tree(OWNER, source.version(), generation)
            .unwrap()
            .focused,
        Some(BUTTON)
    );

    let stale = generation;
    let active = host
        .reconfigure(OWNER, stale, W + 20, H + 10, 1.0)
        .expect("host viewport");
    assert_ne!(active, stale);
    assert!(host
        .accessibility_tree(OWNER, source.version(), stale)
        .is_none());
    assert!(!host.focus(OWNER, source.version(), stale, BUTTON, &mut |_| {}));
    assert!(host.set_viewport_scroll(OWNER, source.version(), active, 0.0, 4.0));

    let suspended = host
        .suspend(OWNER, source.version(), active, &mut |_| {})
        .expect("host suspension");
    let mut pixels = vec![0; ((W + 20) * (H + 10) * 4) as usize];
    assert!(!host.render_to_surface(
        OWNER,
        source.version(),
        suspended,
        &mut pixels,
        ((W + 20) * 4) as usize,
        BUDGET,
    ));
    let resumed = host
        .resume(OWNER, suspended, W, H, 1.0)
        .expect("host resumption");
    let mut pixels = vec![0; (W * H * 4) as usize];
    assert!(host.render_to_surface(
        OWNER,
        source.version(),
        resumed,
        &mut pixels,
        (W * 4) as usize,
        BUDGET,
    ));
    host.retire(OWNER, source.version(), resumed, &mut |_| {})
        .expect("host retirement");
    assert!(host
        .accessibility_tree(OWNER, source.version(), resumed)
        .is_none());
}
