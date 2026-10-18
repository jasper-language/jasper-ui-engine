//! Host-neutral semantics snapshot and focus traversal.
//! Jasper 组件层负责角色/文案/动作语义；这里只承载、校验并供宿主/AccessKit 后续消费。

use crate::input::{KeyEvent, KeyPhase};
use crate::scene::{Node, NodeId, NodeKind, Scene};

/// Coarse accessibility role. Jasper UI may add roles later; unknown values stay explicit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Container,
    Text,
    Button,
    TextField,
    Checkbox,
    Radio,
    Switch,
    Slider,
    Image,
    Link,
    List,
    ListItem,
    Dialog,
    Menu,
    MenuItem,
    ScrollView,
    Unknown,
}

/// Semantics supplied by the Jasper UI layer for one node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Semantics {
    pub role: Role,
    pub label: String,
    pub description: String,
    pub value: String,
    pub disabled: bool,
    pub focusable: bool,
    pub hidden: bool,
}

impl Semantics {
    pub fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }
}

/// One node in the accessibility snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticsNode {
    pub node_id: NodeId,
    pub role: Role,
    pub label: String,
    pub description: String,
    pub value: String,
    pub disabled: bool,
    pub focusable: bool,
    pub hidden: bool,
    pub children: Vec<SemanticsNode>,
}

/// Accessibility snapshot derived from the instance scene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticsTree {
    pub root: SemanticsNode,
}

impl SemanticsTree {
    pub fn from_scene(scene: &Scene) -> Self {
        Self {
            root: snapshot(&scene.root),
        }
    }

    pub fn find(&self, node_id: NodeId) -> Option<&SemanticsNode> {
        find(&self.root, node_id)
    }

    /// Pre-order focusable node IDs; hidden/disabled nodes are excluded.
    pub fn focusables(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        collect_focusables(&self.root, &mut out);
        out
    }
}

fn snapshot(node: &Node) -> SemanticsNode {
    let mut semantics = node.semantics.clone();
    if let NodeKind::Text(text) = &node.kind {
        if semantics.role == Role::Container {
            semantics.role = Role::Text;
        }
        if semantics.label.is_empty() {
            semantics.label = text.clone();
        }
    }
    SemanticsNode {
        node_id: node.id,
        role: semantics.role,
        label: semantics.label,
        description: semantics.description,
        value: semantics.value,
        disabled: semantics.disabled,
        focusable: semantics.focusable,
        hidden: semantics.hidden,
        children: node.children.iter().map(snapshot).collect(),
    }
}

fn find(node: &SemanticsNode, node_id: NodeId) -> Option<&SemanticsNode> {
    if node.node_id == node_id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, node_id))
}

fn collect_focusables(node: &SemanticsNode, out: &mut Vec<NodeId>) {
    if node.hidden {
        return;
    }
    if node.focusable && !node.disabled {
        out.push(node.node_id);
    }
    for child in &node.children {
        collect_focusables(child, out);
    }
}

/// Focus order and current focus for one scene snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusManager {
    order: Vec<NodeId>,
    focused: Option<NodeId>,
}

impl FocusManager {
    pub fn new(tree: &SemanticsTree) -> Self {
        Self {
            order: tree.focusables(),
            focused: None,
        }
    }

    pub fn order(&self) -> &[NodeId] {
        &self.order
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    pub fn focus(&mut self, node_id: NodeId) -> Result<(), FocusError> {
        if self.order.contains(&node_id) {
            self.focused = Some(node_id);
            Ok(())
        } else {
            Err(FocusError::NotFocusable(node_id))
        }
    }

    pub fn blur(&mut self) {
        self.focused = None;
    }

    pub fn focus_next(&mut self) -> Option<NodeId> {
        self.step(1)
    }

    pub fn focus_prev(&mut self) -> Option<NodeId> {
        self.step(-1)
    }

    /// Handles Tab / Shift+Tab on key-down; returns whether focus moved.
    pub fn handle_key(&mut self, event: &KeyEvent) -> bool {
        if event.phase != KeyPhase::Down || event.key != "Tab" {
            return false;
        }
        let before = self.focused;
        if event.modifiers.shift {
            self.focus_prev();
        } else {
            self.focus_next();
        }
        self.focused != before
    }

    fn step(&mut self, delta: isize) -> Option<NodeId> {
        if self.order.is_empty() {
            self.focused = None;
            return None;
        }
        let index = self
            .focused
            .and_then(|id| self.order.iter().position(|item| *item == id));
        let next = match (index, delta.is_negative()) {
            (None, _) => 0,
            (Some(current), true) => (current + self.order.len() - 1) % self.order.len(),
            (Some(current), false) => (current + 1) % self.order.len(),
        };
        self.focused = Some(self.order[next]);
        self.focused
    }
}

/// Focus failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusError {
    NotFocusable(NodeId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;

    fn scene() -> Scene {
        Scene::new(
            Node::container(NodeId(1))
                .child(
                    Node::text(NodeId(2), "name")
                        .with_semantics(Semantics::default().focusable(true)),
                )
                .child(
                    Node::rect(NodeId(3)).with_semantics(
                        Semantics::default()
                            .role(Role::Button)
                            .label("OK")
                            .focusable(true),
                    ),
                )
                .child(
                    Node::rect(NodeId(4)).with_semantics(
                        Semantics::default()
                            .role(Role::Button)
                            .label("disabled")
                            .focusable(true)
                            .disabled(true),
                    ),
                ),
        )
    }

    #[test]
    fn infers_text_label_and_excludes_disabled() {
        let tree = SemanticsTree::from_scene(&scene());
        assert_eq!(tree.find(NodeId(2)).unwrap().label, "name");
        assert_eq!(tree.find(NodeId(3)).unwrap().role, Role::Button);
        assert_eq!(tree.focusables(), vec![NodeId(2), NodeId(3)]);
    }

    #[test]
    fn tab_and_shift_tab_cycle() {
        let tree = SemanticsTree::from_scene(&scene());
        let mut focus = FocusManager::new(&tree);
        let event = |shift| KeyEvent {
            key: "Tab".into(),
            phase: KeyPhase::Down,
            modifiers: Modifiers {
                shift,
                ..Default::default()
            },
            repeat: false,
        };
        assert!(focus.handle_key(&event(false)));
        assert_eq!(focus.focused(), Some(NodeId(2)));
        assert!(focus.handle_key(&event(false)));
        assert_eq!(focus.focused(), Some(NodeId(3)));
        assert!(focus.handle_key(&event(true)));
        assert_eq!(focus.focused(), Some(NodeId(2)));
        assert_eq!(
            focus.focus(NodeId(4)),
            Err(FocusError::NotFocusable(NodeId(4)))
        );
    }
}
