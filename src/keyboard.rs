//! Keyboard routing on top of the host-neutral focus/semantics model.
//! 键盘事件先交给焦点节点；这里只产出意图（聚焦、激活、关闭），不绑定组件实现。

use crate::input::{CompositionEvent, InputEvent, KeyEvent, KeyPhase};
use crate::resource::ResourceGate;
use crate::composition_identity::{UiCompositionSlot, UiCompositionToken, UiPreeditView};
use crate::input_identity::{AdmittedUiInput, UiInputBinding, UiInputContext, UiInputTicket};
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

/// One routed text/IME sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRoute {
    pub target: Option<NodeId>,
    pub consumed: bool,
}

/// One routed IME composition update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionRoute {
    pub event: CompositionEvent,
    pub target: Option<NodeId>,
    pub consumed: bool,
}

/// Focus-aware keyboard router.
///
/// The router owns focus order and command intents only. Text and composition
/// events are routed to the focused editor, but selection/editing state stays
/// with the component.
// BEGIN VERIFIED KEYBOARD STORAGE TYPE
#[derive(Clone, Debug)]
pub struct KeyboardRouter {
    focus: FocusManager,
}
// END VERIFIED KEYBOARD STORAGE TYPE

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

    pub fn focus(&mut self, node: NodeId) -> Result<(), crate::semantics::FocusError> {
        self.focus.focus(node)
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

    /// Routes committed text to the focused text field.
    pub fn route_text(&self, text: &str, tree: &SemanticsTree) -> TextRoute {
        let target = self.focus.focused();
        let consumed =
            target.is_some_and(|node| is_editable_target(tree, node) && !text.is_empty());
        TextRoute { target, consumed }
    }

    /// Routes an IME composition update to the focused text field.
    pub fn route_composition(
        &self,
        event: CompositionEvent,
        tree: &SemanticsTree,
    ) -> CompositionRoute {
        let target = self.focus.focused();
        let consumed = target.is_some_and(|node| is_editable_target(tree, node));
        CompositionRoute {
            event,
            target,
            consumed,
        }
    }
}

// BEGIN VERIFIED KEYBOARD FOCUS KERNEL
impl KeyboardRouter {
    pub fn focused(&self) -> Option<NodeId> {
        self.focus.focused()
    }
    pub fn blur(&mut self) {
        self.focus.blur();
    }

    /// Returns the previous focus and clears it, preserving traversal order.
    /// 取回原焦点并清除，保持遍历顺序；宿主可据此构造失焦意图。
    /// This does not identify an IME owner or deliver cancellation to an application.
    /// 此方法不代表输入法组合拥有者，也不负责向应用投递取消事件。
    pub fn take_focus(&mut self) -> Option<NodeId> {
        let previous = self.focus.focused();
        self.focus.blur();
        previous
    }
}
// END VERIFIED KEYBOARD FOCUS KERNEL

// BEGIN VERIFIED INPUT RETIRE KERNEL
impl KeyboardRouter {
    /// Host-only close step: deny future admission before extracting focus.
    /// 宿主关闭步骤：先永久封闭准入，再取回失焦目标。
    /// The host must pair this gate with this router and gate every input entry.
    /// 宿主必须保证 gate 与路由器的归属匹配，并在所有输入入口检查准入。
    /// This returns an intent target; IME/pointer cancellation and delivery are separate.
    /// 返回值仅供构造失焦意图；预编辑、指针取消和外部投递另行处理。
    pub fn retire_focus(&mut self, gate: &mut ResourceGate) -> Option<NodeId> {
        gate.retire();
        self.take_focus()
    }
}
// END VERIFIED INPUT RETIRE KERNEL

// BEGIN VERIFIED ROLE ACTIVATION KERNEL
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
// END VERIFIED ROLE ACTIVATION KERNEL

// BEGIN VERIFIED COMMAND MODIFIER KERNEL
fn no_command_modifier(event: &KeyEvent) -> bool {
    !event.modifiers.control && !event.modifiers.alt && !event.modifiers.meta
}
// END VERIFIED COMMAND MODIFIER KERNEL

fn focused_role(tree: &SemanticsTree, node: NodeId) -> Option<Role> {
    tree.find(node).map(|item| item.role)
}

fn is_editable_target(tree: &SemanticsTree, node: NodeId) -> bool {
    tree.find(node)
        .is_some_and(|item| item.role == Role::TextField && !item.disabled && !item.hidden)
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
    fn host_retirement_closes_gate_and_extracts_focus_once() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        router.focus(NodeId(4)).unwrap();
        let mut gate = ResourceGate::new(7, 42);
        let old = gate.issue(7).unwrap();
        assert_eq!(router.retire_focus(&mut gate), Some(NodeId(4)));
        assert_eq!(router.focused(), None);
        assert!(!gate.allows(7, old));
        assert_eq!(gate.issue(7), None);
        assert_eq!(router.retire_focus(&mut gate), None);
    }

    #[test]
    fn take_focus_returns_target_once_and_keeps_navigation_usable() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        assert_eq!(router.take_focus(), None);
        router.focus(NodeId(4)).unwrap();
        assert_eq!(router.take_focus(), Some(NodeId(4)));
        assert_eq!(router.focused(), None);
        assert_eq!(router.take_focus(), None);
        let next = router.route(key("Tab"), &tree);
        assert_eq!(next.action, KeyAction::Focus(Some(NodeId(2))));
        router.blur();
        assert_eq!(router.focused(), None);
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

    #[test]
    fn routes_text_and_composition_to_a_focused_text_field() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        router.focus(NodeId(4)).unwrap();

        assert_eq!(
            router.route_text("你好", &tree),
            TextRoute {
                target: Some(NodeId(4)),
                consumed: true
            }
        );
        assert_eq!(
            router.route_composition(
                CompositionEvent {
                    text: "ni".into(),
                    start: 0,
                    end: 2
                },
                &tree
            ),
            CompositionRoute {
                event: CompositionEvent {
                    text: "ni".into(),
                    start: 0,
                    end: 2
                },
                target: Some(NodeId(4)),
                consumed: true
            }
        );
    }

    #[test]
    fn text_is_not_consumed_by_non_editable_targets_or_empty_input() {
        let tree = tree();
        let mut router = KeyboardRouter::new(&tree);
        router.focus(NodeId(2)).unwrap();
        assert!(!router.route_text("x", &tree).consumed);

        router.focus(NodeId(4)).unwrap();
        assert!(!router.route_text("", &tree).consumed);
    }
}

// BEGIN VERIFIED KEYBOARD ADMITTED INPUT KERNEL
impl KeyboardRouter {
    /// Consume an admitted payload only while its target still owns keyboard focus.
    /// 仅当准入目标仍持有键盘焦点时消费负载；返回借用同时锁定路由器和绑定。
    /// This filters ownership, not payload semantics or editable-role policy.
    /// 此入口过滤归属，不替代内容校验或可编辑角色策略；旧底层路由须由可信宿主调用。
    pub fn route_admitted<'a, T>(
        &'a self,
        admitted: &'a AdmittedUiInput<'a, T>,
    ) -> Option<&'a T> {
        match self.focused() {
            Some(target) if target.0 == admitted.target().0 => Some(admitted.payload()),
            _ => None,
        }
    }

    /// Host teardown denies new payloads before clearing focus and returning its old target.
    /// 宿主关闭时先拒绝新负载，再清空焦点并返还旧目标；不等于外部任务已取消。
    pub fn retire_input_binding(&mut self, binding: &mut UiInputBinding) -> Option<NodeId> {
        binding.retire();
        self.take_focus()
    }
}
// END VERIFIED KEYBOARD ADMITTED INPUT KERNEL

// BEGIN VERIFIED KEYBOARD INPUT TRANSFER KERNEL
impl KeyboardRouter {
    /// Coordinate focus selection and input generation rotation under exclusive access.
    /// 独占访问下协调焦点选择与输入代数轮换；拒绝时两个对象均保持原状态。
    /// Trusted host supplies the next incarnation/document context; no callbacks run here.
    /// 可信宿主提供下一代目标/文档上下文；本函数无外部回调，不执行外部任务取消。
    pub fn transfer_input_focus(
        &mut self,
        binding: &mut UiInputBinding,
        caller_session: u64,
        ticket: UiInputTicket,
        next: UiInputContext,
    ) -> Option<UiInputTicket> {
        match self.focused() {
            Some(target) if target.0 == ticket.context.target.0 => {}
            _ => return None,
        }
        if !self.focus.can_focus(next.target) {
            return None;
        }
        match binding.replace(caller_session, ticket, next) {
            Some(fresh) => {
                // Eligibility was checked on this unchanged, exclusively borrowed order.
                // 已在同一份未改变、独占借用的遍历顺序上预检；此选择必定成功。
                let _ = self.focus.focus(next.target);
                Some(fresh)
            }
            None => None,
        }
    }
}
// END VERIFIED KEYBOARD INPUT TRANSFER KERNEL

// BEGIN VERIFIED KEYBOARD INPUT OPEN CLOSE KERNEL
impl KeyboardRouter {
    /// Publish the first focus and input binding together; rejection changes neither.
    /// 同步发布初始焦点与输入绑定；拒绝时两个对象均保持原状，不覆盖活动焦点。
    pub fn begin_input_focus(
        &mut self,
        binding: &mut UiInputBinding,
        caller_session: u64,
        context: UiInputContext,
    ) -> Option<UiInputTicket> {
        if self.focused().is_some() || !self.focus.can_focus(context.target) {
            return None;
        }
        match binding.bind(caller_session, context) {
            Some(ticket) => {
                let _ = self.focus.focus(context.target);
                Some(ticket)
            }
            None => None,
        }
    }

    /// Close a matching active focus and return the complete old owner as a cancellation intent.
    /// 关闭精确匹配的活动焦点并返回完整旧归属作为取消意图；不把旧回调投向新节点。
    /// The host must deliver cancellation to the old owner; this does not stop external work.
    /// 宿主须向旧归属投递取消；此处不终止外部任务，旧票据也不能借此关闭后继。
    pub fn close_input_focus(
        &mut self,
        binding: &mut UiInputBinding,
        caller_session: u64,
        ticket: UiInputTicket,
    ) -> Option<UiInputTicket> {
        match self.focused() {
            Some(target) if target.0 == ticket.context.target.0 => {}
            _ => return None,
        }
        if binding.close(caller_session, ticket) {
            let _ = self.take_focus();
            Some(ticket)
        } else {
            None
        }
    }
}
// END VERIFIED KEYBOARD INPUT OPEN CLOSE KERNEL


// BEGIN VERIFIED KEYBOARD PREEDIT CLOSE KERNEL
impl KeyboardRouter {
    /// Close the exact active preedit owner, focus and binding, then clear matching UI text.
    /// 关闭精确的活动预编辑归属、焦点和绑定，再清除匹配UI文本；失败保持四对象。
    /// Returns old native cancellation correlation only; host delivery/ack remains external.
    /// 只返回旧原生取消关联；宿主投递及回执仍在此同步操作之外。
    pub fn close_input_preedit(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken) -> Option<UiCompositionToken> {
        if !slot.accepts(binding, caller, token) {
            return None;
        }
        match self.close_input_focus(binding, caller, token.owner) {
            Some(_) => {
                let _ = slot.cancel_preedit(view, caller, token);
                Some(token)
            },
            None => None,
        }
    }
}
// END VERIFIED KEYBOARD PREEDIT CLOSE KERNEL


// BEGIN VERIFIED PREEDIT LIFECYCLE DISPATCH KERNEL
impl KeyboardRouter {
    /// Handle qualified lifecycle events for an active composition.
    /// 处理具备完整归属的活动组合生命周期事件：失焦关闭输入，IME禁用只取消组合。
    pub fn handle_preedit_lifecycle(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken, event: &InputEvent) -> Option<UiCompositionToken> {
        match event {
            InputEvent::Focus(focus) if !focus.focused => self.close_input_preedit(binding, slot, view, caller, token),
            InputEvent::ImeEnabled(false) => {
                if !slot.accepts(binding, caller, token) { return None; }
                let _ = slot.cancel_preedit(view, caller, token);
                Some(token)
            },
            _ => None,
        }
    }
}
// END VERIFIED PREEDIT LIFECYCLE DISPATCH KERNEL

// BEGIN VERIFIED INPUT LIFECYCLE DISPATCH KERNEL
impl KeyboardRouter {
    /// Dispatch qualified lifecycle for both idle and composing input ownership.
    /// 为无组合和活动组合输入统一分发生命周期；不为宿主创建或认证归属。
    /// An orphaned view with no composition rejects unchanged and requires explicit cleanup.
    /// 无组合却仍有呈现的异常状态保持原样拒绝，须由宿主显式清理。
    pub fn handle_input_lifecycle(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, owner: UiInputTicket, event: &InputEvent) -> Option<(UiInputTicket, Option<UiCompositionToken>)> {
        if !binding.accepts(caller, owner) { return None; }
        match slot.current_token() {
            Some(token) => match self.handle_preedit_lifecycle(binding, slot, view, caller, token, event) {
                Some(cancelled) => Some((owner, Some(cancelled))),
                None => None,
            },
            None => {
                if view.current().is_some() { return None; }
                match event {
                    InputEvent::Focus(focus) if !focus.focused => match self.close_input_focus(binding, caller, owner) {
                        Some(closed) => Some((closed, None)),
                        None => None,
                    },
                    _ => None,
                }
            },
        }
    }
}
// END VERIFIED INPUT LIFECYCLE DISPATCH KERNEL
