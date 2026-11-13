//! Own per-node scene event projection; native targeting/default actions stay in Blitz.
use crate::{native_events::NativeEventReply, native_view::PointerAction};
use jasper_ui_engine::{
    scene::NodeId,
    semantics::{Role, Semantics},
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneEventKind {
    Pointer(PointerAction),
    Activate,
}
/// Host-routed logical mouse sample; platform identity/capture remain host-owned.
pub struct ScenePointerInput {
    pub phase: PointerAction,
    pub position: (f32, f32),
    pub primary_pressed: bool,
    pub modifiers: jasper_ui_engine::input::Modifiers,
}
pub struct SceneEvent {
    pub scene_version: u64,
    pub node: NodeId,
    pub kind: SceneEventKind,
    /// View-local logical coordinates; not OS screen coordinates.
    pub position: (f32, f32),
    /// State from the native sample, including Move/Up/Cancel.
    pub primary_pressed: bool,
    pub modifiers: jasper_ui_engine::input::Modifiers,
}
/// Raw Framework semantics and ordered source children; host builds its native API tree.
/// Names/roles are preserved, not recomputed from backend DOM or copied display text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneSemanticsNode {
    pub node: NodeId,
    pub semantics: Semantics,
    pub children: Vec<NodeId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneSemanticsSnapshot {
    pub scene_version: u64,
    pub root: NodeId,
    pub nodes: Vec<SceneSemanticsNode>,
}
/// Semantic action identity has no pointer coordinates or backend target.
pub struct SceneActivationEvent {
    pub scene_version: u64,
    pub node: NodeId,
}
/// Native scene focus only; no window focus or external text-service lease ACK.
pub struct SceneFocusEvent {
    pub scene_version: u64,
    pub node: NodeId,
    pub focused: bool,
}
/// Roles the engine activates on a completed primary pointer click (press and release inside
/// the same control) and through `Engine::activate`. Disabled nodes never activate.
/// 引擎在完成的主键指针点击（按下与抬起在同一控件内）及 `Engine::activate` 时激活的角色；禁用节点从不激活。
pub fn role_activates_on_click(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
            | Role::Link
            | Role::MenuItem
            | Role::ListItem
    )
}

/// Keyboard activation of a focused control: Enter for Button/Link/MenuItem, Space for
/// Button/Checkbox/Radio/Switch/MenuItem/ListItem (Space never activates a Link). `key` is the
/// W3C key value; Space is accepted as `" "` or `"Space"`.
/// 聚焦控件的键盘激活：Enter 作用于 Button/Link/MenuItem；Space 作用于 Button/Checkbox/Radio/Switch/
/// MenuItem/ListItem（Space 从不激活 Link）。`key` 为 W3C 键值，空格接受 `" "` 或 `"Space"`。
pub fn role_activates_on_key(role: Role, key: &str) -> bool {
    match key {
        "Enter" => matches!(role, Role::Button | Role::Link | Role::MenuItem),
        " " | "Space" => matches!(
            role,
            Role::Button
                | Role::Checkbox
                | Role::Radio
                | Role::Switch
                | Role::MenuItem
                | Role::ListItem
        ),
        _ => false,
    }
}

/// `ancestor` is `node` or one of its source ancestors; bounded by the map size.
fn source_contains(
    parents: &BTreeMap<NodeId, Option<NodeId>>,
    node: NodeId,
    ancestor: NodeId,
) -> bool {
    let mut current = Some(node);
    let mut remaining = parents.len();
    while let Some(cursor) = current {
        if cursor == ancestor {
            return true;
        }
        if remaining == 0 {
            return false;
        }
        remaining -= 1;
        current = parents.get(&cursor).copied().flatten();
    }
    false
}

pub(crate) struct SceneEventHandler<'a, F> {
    pub version: u64,
    pub suppress_unpaired_up: bool,
    pub nodes: &'a BTreeMap<NodeId, (blitz_dom::NodeId, Option<blitz_dom::NodeId>)>,
    pub semantics: &'a BTreeMap<NodeId, Semantics>,
    pub parents: &'a BTreeMap<NodeId, Option<NodeId>>,
    /// Source node under the native press of this click, if any.
    pub pressed: Option<NodeId>,
    /// E2: source subtree the sample missed through a rounded corner; its nodes are skipped
    /// when the native chain is mapped to a source target (the event goes to the nearest
    /// ancestor outside it).
    /// E2：本次采样因圆角而未命中的源子树；把原生链映射为源目标时跳过其中节点（事件交给其外最近的祖先）。
    pub exclude: Option<NodeId>,
    /// E10: actual sample position when native targeting used a slop-steered point.
    /// E10：原生定位使用了命中扩展修正点时的实际采样位置。
    pub reported_position: Option<(f32, f32)>,
    pub sink: &'a mut F,
}
impl<F: FnMut(SceneEvent) -> NativeEventReply> blitz_dom::EventHandler
    for SceneEventHandler<'_, F>
{
    fn handle_event(
        &mut self,
        chain: &[blitz_dom::NodeId],
        event: &mut blitz_traits::events::DomEvent,
        _: &mut dyn blitz_dom::Document,
        state: &mut blitz_traits::events::EventState,
    ) {
        use blitz_traits::events::DomEventData;
        // Pinned chain remains authoritative. Pointer targets are the nearest source node;
        // activation belongs to the nearest activatable component (role_activates_on_click),
        // including nested caption nodes.
        // Apply lifecycle default suppression even when no source node maps this native target.
        if self.suppress_unpaired_up && matches!(&event.data, DomEventData::PointerUp(_)) {
            state.prevent_default();
        }
        let activation = matches!(&event.data, DomEventData::Click(_));
        // Preserve Framework ancestor hiding for semantic actions; native targeting stays upstream.
        if activation
            && chain.iter().any(|target| {
                self.nodes.iter().any(|(id, (element, text))| {
                    (*element == *target || *text == Some(*target))
                        && self.semantics.get(id).is_some_and(|value| value.hidden)
                })
            })
        {
            return;
        }
        let excluded = |id: &NodeId| {
            self.exclude
                .is_some_and(|root| source_contains(self.parents, *id, root))
        };
        let Some(node) = chain.iter().find_map(|target| {
            self.nodes.iter().find_map(|(id, (element, text))| {
                ((*element == *target || *text == Some(*target))
                    && !excluded(id)
                    && (!activation
                        || self
                            .semantics
                            .get(id)
                            .is_some_and(|s| role_activates_on_click(s.role))))
                .then_some(*id)
            })
        }) else {
            return;
        };
        let (kind, pointer) = match &event.data {
            DomEventData::PointerDown(p) => (SceneEventKind::Pointer(PointerAction::Down), p),
            DomEventData::PointerMove(p) => (SceneEventKind::Pointer(PointerAction::Move), p),
            DomEventData::PointerUp(p) => (SceneEventKind::Pointer(PointerAction::Up), p),
            DomEventData::PointerCancel(p) => (SceneEventKind::Pointer(PointerAction::Cancel), p),
            DomEventData::PointerLeave(p) => (SceneEventKind::Pointer(PointerAction::Leave), p),
            DomEventData::Click(p) => {
                if !self
                    .semantics
                    .get(&node)
                    .is_some_and(|s| role_activates_on_click(s.role) && !s.disabled)
                {
                    return;
                }
                // A click activates only the control the press began in.
                // 只有按下发生在其内部的控件才被点击激活。
                if !self
                    .pressed
                    .is_some_and(|pressed| source_contains(self.parents, pressed, node))
                {
                    return;
                }
                (SceneEventKind::Activate, p)
            }
            _ => return,
        };
        let reply = (self.sink)(SceneEvent {
            scene_version: self.version,
            node,
            kind,
            position: self
                .reported_position
                .unwrap_or((pointer.coords.client_x, pointer.coords.client_y)),
            primary_pressed: pointer
                .buttons
                .contains(blitz_traits::events::MouseEventButtons::Primary),
            modifiers: jasper_ui_engine::input::Modifiers {
                shift: pointer.mods.contains(keyboard_types::Modifiers::SHIFT),
                control: pointer.mods.contains(keyboard_types::Modifiers::CONTROL),
                alt: pointer.mods.contains(keyboard_types::Modifiers::ALT),
                meta: pointer.mods.contains(keyboard_types::Modifiers::META),
            },
        });
        if reply.prevent_default {
            state.prevent_default();
        }
        if reply.stop_propagation {
            state.stop_propagation();
        }
    }
}

/// Final-state notifications only; no external input-service or OS acknowledgement.
pub(crate) fn notify_focus_transition(
    version: u64,
    before: Option<NodeId>,
    after: Option<NodeId>,
    sink: &mut impl FnMut(SceneFocusEvent),
) {
    if before == after {
        return;
    }
    if let Some(node) = before {
        sink(SceneFocusEvent {
            scene_version: version,
            node,
            focused: false,
        });
    }
    if let Some(node) = after {
        sink(SceneFocusEvent {
            scene_version: version,
            node,
            focused: true,
        });
    }
}

/// Native command key projected onto a source node, never an external text-service commit.
pub struct SceneKeyEvent {
    pub scene_version: u64,
    pub node: NodeId,
    pub key: String,
    pub code: String,
    pub pressed: bool,
    pub repeat: bool,
    pub composing: bool,
    pub modifiers: jasper_ui_engine::input::Modifiers,
    pub location: crate::native_keys::KeyLocation,
}
pub(crate) struct SceneKeyHandler<'a, F> {
    pub version: u64,
    pub node: NodeId,
    pub element: blitz_dom::NodeId,
    pub sink: &'a mut F,
}
impl<F: FnMut(SceneKeyEvent) -> NativeEventReply> blitz_dom::EventHandler
    for SceneKeyHandler<'_, F>
{
    fn handle_event(
        &mut self,
        chain: &[blitz_dom::NodeId],
        event: &mut blitz_traits::events::DomEvent,
        _: &mut dyn blitz_dom::Document,
        state: &mut blitz_traits::events::EventState,
    ) {
        use blitz_traits::events::DomEventData;
        let (key, pressed) = match &event.data {
            DomEventData::KeyDown(key) => (key, true),
            DomEventData::KeyUp(key) => (key, false),
            _ => return,
        };
        if !chain.contains(&self.element) {
            return;
        }
        let reply = (self.sink)(SceneKeyEvent {
            scene_version: self.version,
            node: self.node,
            key: key.key.to_string(),
            code: key.code.to_string(),
            pressed,
            repeat: key.is_auto_repeating,
            composing: key.is_composing,
            modifiers: jasper_ui_engine::input::Modifiers {
                shift: key.modifiers.contains(keyboard_types::Modifiers::SHIFT),
                control: key.modifiers.contains(keyboard_types::Modifiers::CONTROL),
                alt: key.modifiers.contains(keyboard_types::Modifiers::ALT),
                meta: key.modifiers.contains(keyboard_types::Modifiers::META),
            },
            location: match key.location {
                keyboard_types::Location::Standard => crate::native_keys::KeyLocation::Standard,
                keyboard_types::Location::Left => crate::native_keys::KeyLocation::Left,
                keyboard_types::Location::Right => crate::native_keys::KeyLocation::Right,
                keyboard_types::Location::Numpad => crate::native_keys::KeyLocation::Numpad,
            },
        });
        if reply.prevent_default {
            state.prevent_default();
        }
        if reply.stop_propagation {
            state.stop_propagation();
        }
    }
}
