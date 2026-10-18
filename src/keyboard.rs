//! Keyboard routing on top of the host-neutral focus/semantics model.
//! 键盘事件先交给焦点节点；这里只产出意图（聚焦、激活、关闭），不绑定组件实现。

use crate::input::{KeyEvent, KeyPhase};
use crate::scene::NodeId;
use crate::semantics::{FocusManager, Role, SemanticsTree};

/// What the engine understood from one key event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Tab/Shift+Tab moved focus (or kept it when only one node is focusable).
    Focus(Option<NodeId>),
    /// Enter/Space on an activatable role.
    Activate { target: NodeId, role: Role },
    /// Escape: the host/component may close a dialog, menu or popup.
    Dismiss,
    /// Nothing was consumed; the focused component or host shortcut layer
    /// should handle the original event.
    Unhandled,
}

/// One routed key event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRoute {
    pub event: KeyEvent,
    /// Focused node after routing; `None` when nothing is focused.
    pub target: Option<NodeId>,
    pub consumed: bool,
    pub action: KeyAction,
}

/// Focus-aware keyboard router.
///
/// The router owns focus order only. Text input and IME composition stay
/// separate because they target an editor and have their own selection model.
#[derive(Clone, Debug)]
pub struct KeyboardRouter {
    focus: FocusManager,
}

impl KeyboardRouter {
    pub fn new(tree: &SemanticsTree) -> Self {
        Self {
            focus: FocusManager::new(tree),
        }
    }

    /// Rebuilds focus order after the scene changed. Keeps focus only when the
    /// previously focused node is still focusable.
    pub fn refresh(&mut self, tree: &SemanticsTree) {
        let previous = self.focus.focused();
        self.focus = FocusManager::new(tree);
        if let Some(node) = previous {
            let _ = self.focus.focus(node);
        }
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.focus.focused()
    }

    pub fn focus(&mut self, node: NodeId) -> Result<(), crate::semantics::FocusError> {
        self.focus.focus(node)
    }

    pub fn blur(&mut self) {
        self.focus.blur();
    }

    /// Routes one key event and returns the resulting intent.
    pub fn route(&mut self, event: KeyEvent, tree: &SemanticsTree) -> KeyRoute {
        let target = self.focus.focused();
        let mut consumed = false;
        let mut action = KeyAction::Unhandled;

        if event.phase == KeyPhase::Down {
            // Tab is navigation, not a component shortcut.
            if event.key == "Tab" && no_command_modifier(&event) {
                self.focus.handle_key(&event);
                consumed = true;
                action = KeyAction::Focus(self.focus.focused());
            } else if event.key == "Escape" && no_command_modifier(&event) {
                consumed = true;
                action = KeyAction::Dismiss;
            } else if no_command_modifier(&event) {
                if let Some(node) = target {
                    if let Some(role) = focused_role(tree, node) {
                        let activation_key = event.key == "Enter"
                            || (event.key == "Space" && role.activates_with_space());
                        if activation_key && role.is_activatable() {
                            consumed = true;
                            action = KeyAction::Activate { target: node, role };
                        }
                    }
                }
            }
        }

        KeyRoute {
            event,
            target: self.focus.focused(),
            consumed,
            action,
        }
    }
}

impl Role {
    /// Roles a keyboard Enter/Space should activate.
    pub fn is_activatable(self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::Checkbox
                | Role::Radio
                | Role::Switch
                | Role::Link
                | Role::MenuItem
        )
    }

    /// Space activates most controls, but not links (Space scrolls a page).
    pub fn activates_with_space(self) -> bool {
        matches!(
            self,
            Role::Button | Role::Checkbox | Role::Radio | Role::Switch | Role::MenuItem
        )
    }
}

fn no_command_modifier(event: &KeyEvent) -> bool {
    !event.modifiers.control && !event.modifiers.alt && !event.modifiers.meta
}

fn focused_role(tree: &SemanticsTree, node: NodeId) -> Option<Role> {
    tree.find(node).map(|item| item.role)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;
    use crate::scene::{Node, Scene};
    use crate::semantics::{Semantics, SemanticsTree};

    fn key(name: &str) -> KeyEvent {
        KeyEvent {
            key: name.to_string(),
            phase: KeyPhase::Down,
            modifiers: Modifiers::default(),
            repeat: false,
        }
    }

    fn tree() -> SemanticsTree {
        SemanticsTree::from_scene(&Scene::new(
            Node::container(NodeId(1))
                .child(
                    Node::rect(NodeId(2))
                        .with_semantics(Semantics::default().role(Role::Button).focusable(true)),
                )
                .child(
                    Node::rect(NodeId(3))
                        .with_semantics(Semantics::default().role(Role::Checkbox).focusable(true)),
                )
                .child(
                    Node::rect(NodeId(4))
                        .with_semantics(Semantics::default().role(Role::TextField).focusable(true)),
                ),
        ))
    }

    #[test]
    fn tab_moves_focus_and_shift_tab_moves_back() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        let first = router.route(key("Tab"), &tree);
        assert!(first.consumed);
        assert_eq!(first.action, KeyAction::Focus(Some(NodeId(2))));

        let mut shift = key("Tab");
        shift.modifiers.shift = true;
        let back = router.route(shift, &tree);
        assert_eq!(back.action, KeyAction::Focus(Some(NodeId(4))));
    }

    #[test]
    fn enter_activates_focused_button() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        router.focus(NodeId(2)).unwrap();
        let route = router.route(key("Enter"), &tree);
        assert!(route.consumed);
        assert_eq!(
            route.action,
            KeyAction::Activate {
                target: NodeId(2),
                role: Role::Button
            }
        );
    }

    #[test]
    fn space_activates_checkbox_but_not_text_field() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        router.focus(NodeId(3)).unwrap();
        assert!(matches!(
            router.route(key("Space"), &tree).action,
            KeyAction::Activate { .. }
        ));

        router.focus(NodeId(4)).unwrap();
        let route = router.route(key("Space"), &tree);
        assert!(!route.consumed);
        assert_eq!(route.action, KeyAction::Unhandled);
    }

    #[test]
    fn escape_is_a_dismiss_intent() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        let route = router.route(key("Escape"), &tree);
        assert!(route.consumed);
        assert_eq!(route.action, KeyAction::Dismiss);
    }

    #[test]
    fn command_modifiers_and_key_up_are_left_to_the_host() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        router.focus(NodeId(2)).unwrap();

        let mut shortcut = key("Enter");
        shortcut.modifiers.control = true;
        assert!(!router.route(shortcut, &tree).consumed);

        let mut up = key("Enter");
        up.phase = KeyPhase::Up;
        assert!(!router.route(up, &tree).consumed);
    }

    #[test]
    fn focus_survives_refresh_when_still_focusable() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        router.focus(NodeId(2)).unwrap();
        router.refresh(&tree);
        assert_eq!(router.focused(), Some(NodeId(2)));
    }
}
