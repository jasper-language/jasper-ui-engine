//! Instance scene tree with stable node identities and incremental patches.
//! 实例层场景树；不是 Jasper 的声明层组件树，也不是 Web DOM。

use crate::style::Style;

/// Stable identity of a node within a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

/// What a node represents for the engine.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Container,
    Text(String),
    Rect,
}

/// One node in the instance scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub style: Style,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(id: NodeId, kind: NodeKind) -> Self {
        Self {
            id,
            kind,
            style: Style::default(),
            children: Vec::new(),
        }
    }

    pub fn container(id: NodeId) -> Self {
        Self::new(id, NodeKind::Container)
    }

    pub fn text(id: NodeId, text: impl Into<String>) -> Self {
        Self::new(id, NodeKind::Text(text.into()))
    }

    pub fn rect(id: NodeId) -> Self {
        Self::new(id, NodeKind::Rect)
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: Vec<Node>) -> Self {
        self.children = children;
        self
    }

    pub fn text_content(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// A scene is rooted at a single container node.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub root: Node,
}

impl Scene {
    pub fn new(root: Node) -> Self {
        Self { root }
    }

    pub fn find(&self, id: NodeId) -> Option<&Node> {
        find_node(&self.root, id)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.find(id).is_some()
    }

    /// Applies one incremental patch. Patches are not transactional yet;
    /// callers that need atomicity should build a batch API in a later version.
    pub fn apply(&mut self, patch: ScenePatch) -> Result<(), SceneError> {
        match patch {
            ScenePatch::Insert {
                parent,
                index,
                node,
            } => {
                if self.contains(node.id) {
                    return Err(SceneError::DuplicateNode(node.id));
                }
                let parent_node = find_node_mut(&mut self.root, parent)
                    .ok_or(SceneError::NodeNotFound(parent))?;
                let index = index.min(parent_node.children.len());
                parent_node.children.insert(index, node);
                Ok(())
            }
            ScenePatch::UpdateStyle { id, style } => {
                let node = find_node_mut(&mut self.root, id).ok_or(SceneError::NodeNotFound(id))?;
                node.style = style;
                Ok(())
            }
            ScenePatch::UpdateText { id, text } => {
                let node = find_node_mut(&mut self.root, id).ok_or(SceneError::NodeNotFound(id))?;
                match &mut node.kind {
                    NodeKind::Text(existing) => {
                        *existing = text;
                        Ok(())
                    }
                    _ => Err(SceneError::NotText(id)),
                }
            }
            ScenePatch::Remove { id } => {
                if self.root.id == id {
                    return Err(SceneError::CannotRemoveRoot);
                }
                if remove_node(&mut self.root, id) {
                    Ok(())
                } else {
                    Err(SceneError::NodeNotFound(id))
                }
            }
        }
    }
}

/// Incremental scene operation.
#[derive(Clone, Debug, PartialEq)]
pub enum ScenePatch {
    Insert {
        parent: NodeId,
        index: usize,
        node: Node,
    },
    UpdateStyle {
        id: NodeId,
        style: Style,
    },
    UpdateText {
        id: NodeId,
        text: String,
    },
    Remove {
        id: NodeId,
    },
}

/// Scene patch failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    NodeNotFound(NodeId),
    DuplicateNode(NodeId),
    CannotRemoveRoot,
    NotText(NodeId),
}

fn find_node(node: &Node, id: NodeId) -> Option<&Node> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, id))
}

fn find_node_mut(node: &mut Node, id: NodeId) -> Option<&mut Node> {
    if node.id == id {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_node_mut(child, id))
}

fn remove_node(parent: &mut Node, id: NodeId) -> bool {
    if let Some(index) = parent.children.iter().position(|child| child.id == id) {
        parent.children.remove(index);
        return true;
    }
    parent
        .children
        .iter_mut()
        .any(|child| remove_node(child, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u64) -> NodeId {
        NodeId(value)
    }

    #[test]
    fn applies_incremental_patches() {
        let mut scene = Scene::new(Node::container(id(1)));
        scene
            .apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::text(id(2), "hello"),
            })
            .unwrap();
        assert!(scene.contains(id(2)));
        scene
            .apply(ScenePatch::UpdateText {
                id: id(2),
                text: "world".into(),
            })
            .unwrap();
        assert_eq!(scene.find(id(2)).unwrap().text_content(), Some("world"));
        scene.apply(ScenePatch::Remove { id: id(2) }).unwrap();
        assert!(!scene.contains(id(2)));
    }

    #[test]
    fn rejects_duplicate_and_root_removal() {
        let mut scene = Scene::new(Node::container(id(1)));
        let child = Node::rect(id(2));
        scene
            .apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: child.clone(),
            })
            .unwrap();
        assert_eq!(
            scene.apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: child
            }),
            Err(SceneError::DuplicateNode(id(2)))
        );
        assert_eq!(
            scene.apply(ScenePatch::Remove { id: id(1) }),
            Err(SceneError::CannotRemoveRoot)
        );
    }
}
