//! E11/E12/E13: engine-produced activation by role (pointer click and keyboard), focus on pointer
//! press, new semantic roles in the accessibility tree / AccessKit, and HostRenderer forwarding of
//! node_bounds/activate/key_with_activation.
//! E11/E12/E13：引擎按角色统一产出激活（指针点击与键盘）、指针按下聚焦、新语义角色进入无障碍树与
//! AccessKit，以及 HostRenderer 转发 node_bounds/activate/key_with_activation。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::input::Modifiers;
use jasper_ui_engine::native::host::HostRenderer;
use jasper_ui_engine::native::{
    role_activates_on_click, role_activates_on_key, EventReply, HostKeyInput, KeyLocation,
    PointerAction, SceneEventKind,
};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::semantics::{HeadingLevel, LivePoliteness, Role, Semantics};
use jasper_ui_engine::style::{Color, Display, FlexDirection, Length, Style};

const OWNER: u64 = 41;
const W: u32 = 400;
const H: u32 = 400;

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT")).unwrap()
}

fn control(id: u64, role: Role, disabled: bool) -> Node {
    Node::container(NodeId(id))
        .with_style(
            Style::default()
                .size(Length::Px(120.0), Length::Px(24.0))
                .background(Color::rgb(30, 60, 120)),
        )
        .with_semantics(
            Semantics::default()
                .role(role)
                .label(format!("c{id}"))
                .focusable(true)
                .disabled(disabled),
        )
        .child(Node::text(NodeId(id + 100), format!("caption {id}")))
}

/// Controls 2..=8 of every activatable role, 9 a disabled checkbox, 10 plain text, 11 heading,
/// 12 status, 13 polite live, 14 assertive live, 15 alert, 16 form.
fn scene() -> Scene {
    let roles = [
        (2, Role::Button),
        (3, Role::Checkbox),
        (4, Role::Radio),
        (5, Role::Switch),
        (6, Role::Link),
        (7, Role::MenuItem),
        (8, Role::ListItem),
    ];
    let mut root = Node::container(NodeId(1)).with_style(
        Style::default()
            .size(Length::Px(W as f32), Length::Px(H as f32))
            .display(Display::Flex)
            .direction(FlexDirection::Column),
    );
    for (id, role) in roles {
        root = root.child(control(id, role, false));
    }
    root = root.child(control(9, Role::Checkbox, true));
    root = root.child(Node::text(NodeId(10), "plain"));
    let semantic = |id: u64, role: Role| {
        Node::text(NodeId(id), format!("s{id}")).with_semantics(Semantics::default().role(role))
    };
    Scene::new(
        root.child(semantic(11, Role::Heading(HeadingLevel::new(2).unwrap())))
            .child(semantic(12, Role::Status))
            .child(semantic(13, Role::Live(LivePoliteness::Polite)))
            .child(semantic(14, Role::Live(LivePoliteness::Assertive)))
            .child(semantic(15, Role::Alert))
            .child(semantic(16, Role::Form)),
    )
}

fn engine() -> (Engine, u64) {
    let source = scene();
    let version = source.version();
    (
        Engine::with_fonts(OWNER, &source, &[&font()], W, H).unwrap(),
        version,
    )
}

fn center(engine: &Engine, version: u64, id: u64) -> (f32, f32) {
    let b = engine.node_bounds(OWNER, version, NodeId(id)).unwrap();
    (
        b.origin.x + b.size.width / 2.0,
        b.origin.y + b.size.height / 2.0,
    )
}

/// Press at `from`, release at `to`; returns (activated nodes, focus events (node, focused)).
fn click(
    engine: &mut Engine,
    v: u64,
    from: (f32, f32),
    to: (f32, f32),
) -> (Vec<u64>, Vec<(u64, bool)>) {
    let mut activated = Vec::new();
    let mut focus = Vec::new();
    for (phase, (x, y), pressed) in [
        (PointerAction::Move, from, false),
        (PointerAction::Down, from, true),
        (PointerAction::Move, to, true),
        (PointerAction::Up, to, false),
    ] {
        assert!(engine.pointer(
            OWNER,
            v,
            phase,
            x,
            y,
            pressed,
            &mut |e| {
                if e.kind == SceneEventKind::Activate {
                    activated.push(e.node.0);
                }
                EventReply::default()
            },
            &mut |f| focus.push((f.node.0, f.focused)),
        ));
    }
    (activated, focus)
}

fn key_input(key: &str, repeat: bool, modifiers: Modifiers) -> HostKeyInput<'_> {
    HostKeyInput {
        key,
        code: match key {
            " " => "Space",
            other => other,
        },
        pressed: true,
        repeat,
        composing: false,
        modifiers,
        location: KeyLocation::Standard,
    }
}

fn press(engine: &mut Engine, v: u64, key: &str, repeat: bool, modifiers: Modifiers) -> Vec<u64> {
    let mut activated = Vec::new();
    assert!(engine.key_with_activation(
        OWNER,
        v,
        &key_input(key, repeat, modifiers),
        &mut |_| EventReply::default(),
        &mut |_| {},
        &mut |a| activated.push(a.node.0),
    ));
    activated
}

#[test]
fn role_tables() {
    for role in [
        Role::Button,
        Role::Checkbox,
        Role::Radio,
        Role::Switch,
        Role::Link,
        Role::MenuItem,
        Role::ListItem,
    ] {
        assert!(role_activates_on_click(role));
    }
    for role in [
        Role::Text,
        Role::TextField,
        Role::Container,
        Role::Slider,
        Role::Status,
        Role::Form,
    ] {
        assert!(!role_activates_on_click(role));
    }
    assert!(
        role_activates_on_key(Role::Button, "Enter") && role_activates_on_key(Role::Button, " ")
    );
    assert!(role_activates_on_key(Role::Link, "Enter") && !role_activates_on_key(Role::Link, " "));
    assert!(
        role_activates_on_key(Role::Checkbox, " ")
            && !role_activates_on_key(Role::Checkbox, "Enter")
    );
    assert!(
        role_activates_on_key(Role::ListItem, "Space") && !role_activates_on_key(Role::Button, "a")
    );
    assert_eq!(HeadingLevel::new(0), None);
    assert_eq!(HeadingLevel::new(7), None);
    assert_eq!(HeadingLevel::new(6).map(HeadingLevel::get), Some(6));
}

#[test]
fn pointer_click_activates_every_activatable_role_and_focuses_it() {
    let (mut engine, v) = engine();
    for id in 2..=8 {
        let at = center(&engine, v, id);
        let (activated, focus) = click(&mut engine, v, at, at);
        assert_eq!(activated, vec![id], "role control #{id}");
        assert!(
            focus.contains(&(id, true)),
            "pointer press focuses #{id}: {focus:?}"
        );
        assert_eq!(engine.focused_node(OWNER, v), Some(NodeId(id)));
    }
    // Pressing on the caption text still activates its control.
    let caption = center(&engine, v, 103);
    assert_eq!(click(&mut engine, v, caption, caption).0, vec![3]);
    // Disabled control or plain text: no activation, and (as in a browser) a click outside any
    // focusable control blurs the focused one.
    // 禁用控件或普通文字：不激活；与浏览器一致，点击可聚焦控件之外会使原焦点失焦。
    for id in [9, 10] {
        let at = center(&engine, v, id);
        let (activated, focus) = click(&mut engine, v, at, at);
        assert!(activated.is_empty(), "#{id}");
        assert!(
            focus.iter().all(|(_, focused)| !focused),
            "#{id}: {focus:?}"
        );
        assert_eq!(engine.focused_node(OWNER, v), None);
    }
    // Press in one control, release in another: nothing activates.
    let (a, b) = (center(&engine, v, 3), center(&engine, v, 4));
    assert!(click(&mut engine, v, a, b).0.is_empty());
}

#[test]
fn keyboard_activation_follows_role_keys() {
    let (mut engine, v) = engine();
    let none = Modifiers::default();
    let focus =
        |engine: &mut Engine, id: u64| assert!(engine.focus(OWNER, v, NodeId(id), &mut |_| {}));
    focus(&mut engine, 2);
    assert_eq!(press(&mut engine, v, "Enter", false, none), vec![2]);
    assert_eq!(press(&mut engine, v, " ", false, none), vec![2]);
    assert!(
        press(&mut engine, v, "Enter", true, none).is_empty(),
        "repeat"
    );
    let ctrl = Modifiers {
        control: true,
        ..Modifiers::default()
    };
    assert!(
        press(&mut engine, v, "Enter", false, ctrl).is_empty(),
        "ctrl+Enter"
    );
    focus(&mut engine, 3);
    assert_eq!(press(&mut engine, v, " ", false, none), vec![3]);
    assert!(press(&mut engine, v, "Enter", false, none).is_empty());
    for id in [4, 5, 8] {
        focus(&mut engine, id);
        assert_eq!(press(&mut engine, v, " ", false, none), vec![id]);
    }
    focus(&mut engine, 6);
    assert_eq!(press(&mut engine, v, "Enter", false, none), vec![6]);
    assert!(
        press(&mut engine, v, " ", false, none).is_empty(),
        "Space never activates a link"
    );
    focus(&mut engine, 7);
    assert_eq!(press(&mut engine, v, "Enter", false, none), vec![7]);
    assert_eq!(press(&mut engine, v, " ", false, none), vec![7]);
    // A prevented key default suppresses activation.
    focus(&mut engine, 2);
    let mut activated = Vec::new();
    assert!(engine.key_with_activation(
        OWNER,
        v,
        &key_input("Enter", false, none),
        &mut |_| EventReply {
            prevent_default: true,
            ..EventReply::default()
        },
        &mut |_| {},
        &mut |a| activated.push(a.node.0),
    ));
    assert!(activated.is_empty());
    // The legacy key entry reports no activation channel and keeps working.
    assert!(engine.key(
        OWNER,
        v,
        &key_input("Enter", false, none),
        &mut |_| EventReply::default(),
        &mut |_| {}
    ));
}

#[test]
fn semantic_activation_and_actions_cover_roles() {
    let (engine, v) = engine();
    for id in 2..=8 {
        let mut got = Vec::new();
        assert!(engine.activate(OWNER, v, NodeId(id), &mut |a| got.push(a.node.0)));
        assert_eq!(got, vec![id]);
    }
    assert!(!engine.activate(OWNER, v, NodeId(9), &mut |_| panic!("disabled")));
    assert!(!engine.activate(OWNER, v, NodeId(12), &mut |_| panic!("status")));
    let tree = engine.accessibility_tree(OWNER, v).unwrap();
    let node = |id: u64| tree.nodes.iter().find(|n| n.node == NodeId(id)).unwrap();
    for id in 2..=8 {
        assert!(node(id).actions.activate, "#{id}");
    }
    assert!(!node(9).actions.activate);
    assert_eq!(node(11).role, Role::Heading(HeadingLevel::new(2).unwrap()));
    assert_eq!(node(12).role, Role::Status);
    assert_eq!(node(13).role, Role::Live(LivePoliteness::Polite));
    assert_eq!(node(14).role, Role::Live(LivePoliteness::Assertive));
    assert_eq!(node(15).role, Role::Alert);
    assert_eq!(node(16).role, Role::Form);
}

#[cfg(feature = "accesskit-pending")]
#[test]
fn new_roles_map_to_accesskit_with_level_and_live() {
    use jasper_ui_engine::native::host::accesskit::{Affine, Live, Role as KitRole};
    let (engine, v) = engine();
    let update = engine
        .accessibility_tree(OWNER, v)
        .unwrap()
        .to_accesskit(Affine::IDENTITY)
        .unwrap();
    let kit = |id: u64| &update.nodes.iter().find(|(k, _)| k.0 == id).unwrap().1;
    assert_eq!(kit(11).role(), KitRole::Heading);
    assert_eq!(kit(11).level(), Some(2));
    assert_eq!(
        (kit(12).role(), kit(12).live()),
        (KitRole::Status, Some(Live::Polite))
    );
    assert_eq!(
        (kit(13).role(), kit(13).live()),
        (KitRole::Region, Some(Live::Polite))
    );
    assert_eq!(
        (kit(14).role(), kit(14).live()),
        (KitRole::Region, Some(Live::Assertive))
    );
    assert_eq!(
        (kit(15).role(), kit(15).live()),
        (KitRole::Alert, Some(Live::Assertive))
    );
    assert_eq!(kit(16).role(), KitRole::Form);
    assert_eq!(kit(2).live(), None);
    // The reference consumer accepts the tree.
    let _ = accesskit_consumer::Tree::new(update, true);
}

#[test]
fn host_renderer_forwards_bounds_activation_and_key_activation() {
    let source = scene();
    let v = source.version();
    let mut renderer = HostRenderer::with_fonts(OWNER, &source, &[&font()], W, H).unwrap();
    let generation = renderer.surface_status(OWNER).unwrap().generation;
    let bounds = renderer
        .node_bounds(OWNER, v, generation, NodeId(3))
        .unwrap();
    assert!(bounds.size.width > 0.0 && bounds.size.height > 0.0);
    assert!(renderer
        .node_bounds(OWNER, v, generation + 1, NodeId(3))
        .is_none());
    assert!(renderer
        .node_bounds(OWNER + 1, v, generation, NodeId(3))
        .is_none());
    let mut got = Vec::new();
    assert!(renderer.activate(OWNER, v, generation, NodeId(3), &mut |a| got.push(a.node.0)));
    assert!(
        !renderer.activate(OWNER, v, generation + 1, NodeId(3), &mut |_| panic!(
            "stale generation"
        ))
    );
    assert!(renderer.focus(OWNER, v, generation, NodeId(5), &mut |_| {}));
    assert!(renderer.key_with_activation(
        OWNER,
        v,
        generation,
        &key_input(" ", false, Modifiers::default()),
        &mut |_| EventReply::default(),
        &mut |_| {},
        &mut |a| got.push(a.node.0),
    ));
    assert_eq!(got, vec![3, 5]);
}
