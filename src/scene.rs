//! Instance scene tree with stable node identities and incremental patches.
//! 实例层场景树；不是 Jasper 的声明层组件树，也不是 Web DOM。

use crate::canvas::Canvas;
use crate::geometry::Point;
use crate::semantics::Semantics;
use crate::style::Style;
use std::collections::BTreeSet;

/// Stable identity of a node within a scene.
// BEGIN VERIFIED NODE ID TYPE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);
// END VERIFIED NODE ID TYPE

/// What a node represents for the engine.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Container,
    Text(String),
    Rect,
    Canvas(Canvas),
}

/// One node in the instance scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub style: Style,
    pub semantics: Semantics,
    /// Child offset applied after layout; scroll containers update this.
    pub scroll: Point,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(id: NodeId, kind: NodeKind) -> Self {
        Self {
            id,
            kind,
            style: Style::default(),
            semantics: Semantics::default(),
            scroll: Point::ZERO,
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

    pub fn canvas(id: NodeId, canvas: Canvas) -> Self {
        Self::new(id, NodeKind::Canvas(canvas))
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_semantics(mut self, semantics: Semantics) -> Self {
        self.semantics = semantics;
        self
    }

    pub fn scroll(mut self, offset: Point) -> Self {
        self.scroll = offset;
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

    pub fn canvas_content(&self) -> Option<&Canvas> {
        match &self.kind {
            NodeKind::Canvas(canvas) => Some(canvas),
            _ => None,
        }
    }
}

/// A scene is rooted at a single container node.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub root: Node,
    version: u64,
}

impl Scene {
    /// Compatibility constructor; public tree mutation is checked at use sites.
    /// 兼容构造入口；公开树可能被修改，更新与布局入口仍须检查有效性。
    pub fn new(root: Node) -> Self {
        Self { root, version: 1 }
    }

    /// Constructs a scene only if every node identity is unique.
    /// 仅当全树节点身份唯一时构造场景；供不可信输入的接入边界使用。
    pub fn try_new(root: Node) -> Result<Self, SceneError> {
        let scene = Self::new(root);
        scene.validate()?;
        Ok(scene)
    }

    /// Rechecks identity uniqueness, including direct edits to the public root.
    /// 重新检查身份唯一性，覆盖对公开 root 的直接修改。
    pub fn validate(&self) -> Result<(), SceneError> {
        collect_unique_ids(&self.root, &mut BTreeSet::new())
    }

    /// Monotonic version; incremented exactly once per successful patch/batch.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn find(&self, id: NodeId) -> Option<&Node> {
        find_node(&self.root, id)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.find(id).is_some()
    }

    /// Applies one patch atomically (clone-on-write for the prototype).
    pub fn apply(&mut self, patch: ScenePatch) -> Result<(), SceneError> {
        self.apply_batch(std::slice::from_ref(&patch))
    }

    /// Applies all patches or none. The version advances once on success.
    pub fn apply_batch(&mut self, patches: &[ScenePatch]) -> Result<(), SceneError> {
        self.apply_batch_at(self.version, patches)
    }

    /// Rejects a stale batch when `expected_version` is not the current version.
    pub fn apply_batch_at(
        &mut self,
        expected_version: u64,
        patches: &[ScenePatch],
    ) -> Result<(), SceneError> {
        let next_version =
            match decide_scene_version(self.version, expected_version, patches.is_empty()) {
                VersionDecision::Stale => {
                    return Err(SceneError::StaleVersion {
                        expected: expected_version,
                        actual: self.version,
                    })
                }
                VersionDecision::Unchanged => return self.validate(),
                VersionDecision::Exhausted => return Err(SceneError::VersionExhausted),
                VersionDecision::Advance(version) => version,
            };
        self.validate()?;
        let mut next = self.clone();
        for patch in patches {
            next.apply_one(patch.clone())?;
        }
        next.version = next_version;
        *self = next;
        Ok(())
    }

    fn apply_one(&mut self, patch: ScenePatch) -> Result<(), SceneError> {
        match patch {
            ScenePatch::Insert {
                parent,
                index,
                node,
            } => {
                // Validate all descendants before any insertion or follow-up patch
                // can observe ambiguous identities. The set is local to staging.
                // 插入前检查全部后代，避免后续补丁观察到歧义身份；集合仅在暂存阶段使用。
                let mut ids = BTreeSet::new();
                collect_unique_ids(&self.root, &mut ids)?;
                collect_unique_ids(&node, &mut ids)?;
                let parent_node = find_node_mut(&mut self.root, parent)
                    .ok_or(SceneError::NodeNotFound(parent))?;
                let index = index.min(parent_node.children.len());
                parent_node.children.insert(index, node);
                Ok(())
            }
            ScenePatch::UpdateSemantics {id,semantics} => {
                let node=find_node_mut(&mut self.root,id).ok_or(SceneError::NodeNotFound(id))?;
                node.semantics=semantics;
                Ok(())
            }
            ScenePatch::UpdateStyle { id, style } => {
                let node = find_node_mut(&mut self.root, id).ok_or(SceneError::NodeNotFound(id))?;
                node.style = style;
                Ok(())
            }
            ScenePatch::SetScroll { id, offset } => {
                let node = find_node_mut(&mut self.root, id).ok_or(SceneError::NodeNotFound(id))?;
                node.scroll = offset;
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
            ScenePatch::SetCanvas { id, canvas } => {
                let node = find_node_mut(&mut self.root, id).ok_or(SceneError::NodeNotFound(id))?;
                match &mut node.kind {
                    NodeKind::Canvas(existing) => {
                        *existing = canvas;
                        Ok(())
                    }
                    _ => Err(SceneError::NotCanvas(id)),
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
    /// Framework-owned semantic state; native backends may require stable component role.
    UpdateSemantics {
        id: NodeId,
        semantics: Semantics,
    },
    UpdateText {
        id: NodeId,
        text: String,
    },
    SetCanvas {
        id: NodeId,
        canvas: Canvas,
    },
    SetScroll {
        id: NodeId,
        offset: Point,
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
    NotCanvas(NodeId),
    StaleVersion {
        expected: u64,
        actual: u64,
    },
    /// No fresh version is available; the scene remains unchanged.
    /// 无可用的新版本；场景保持不变。
    VersionExhausted,
}

// BEGIN VERIFIED VERSION KERNEL
/// Computes the next version without wrapping or saturating.
/// 计算下一版本，不回绕、不饱和；证明脚本直接提取此函数体。
fn next_scene_version(version: u64) -> Option<u64> {
    if version < u64::MAX {
        Some(version + 1)
    } else {
        None
    }
}
// END VERIFIED VERSION KERNEL

// BEGIN VERIFIED TRANSACTION GATE KERNEL
enum VersionDecision {
    Stale,
    Unchanged,
    Exhausted,
    Advance(u64),
}

/// Gives stale versions priority, preserves empty batches and rejects exhaustion.
/// 优先拒绝陈旧版本，空批保持版本，非空批耗尽时拒绝；不修改任何场景状态。
fn decide_scene_version(current: u64, expected: u64, empty: bool) -> VersionDecision {
    if expected != current {
        VersionDecision::Stale
    } else if empty {
        VersionDecision::Unchanged
    } else {
        match next_scene_version(current) {
            Some(next) => VersionDecision::Advance(next),
            None => VersionDecision::Exhausted,
        }
    }
}
// END VERIFIED TRANSACTION GATE KERNEL

/// Traverses iteratively so identity validation itself does not recurse.
/// 迭代遍历，身份校验本身不递归；不代表后续布局/克隆已具备深度资源限制。
fn collect_unique_ids(root: &Node, ids: &mut BTreeSet<NodeId>) -> Result<(), SceneError> {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if !ids.insert(node.id) {
            return Err(SceneError::DuplicateNode(node.id));
        }
        pending.extend(node.children.iter().rev());
    }
    Ok(())
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

    #[test]
    fn version_advances_only_on_success() {
        let mut scene = Scene::new(Node::container(id(1)));
        assert_eq!(scene.version(), 1);
        scene
            .apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::rect(id(2)),
            })
            .unwrap();
        assert_eq!(scene.version(), 2);
        assert_eq!(
            scene.apply(ScenePatch::Remove { id: id(9) }),
            Err(SceneError::NodeNotFound(id(9)))
        );
        assert_eq!(scene.version(), 2);
    }

    #[test]
    fn batch_is_atomic() {
        let mut scene = Scene::new(Node::container(id(1)));
        let result = scene.apply_batch(&[
            ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::rect(id(2)),
            },
            ScenePatch::Remove { id: id(9) },
        ]);
        assert_eq!(result, Err(SceneError::NodeNotFound(id(9))));
        assert!(!scene.contains(id(2)));
        assert_eq!(scene.version(), 1);
    }

    #[test]
    fn checked_construction_and_legacy_updates_reject_duplicate_descendants() {
        let root = Node::container(id(1)).child(Node::container(id(2)).child(Node::rect(id(1))));
        assert_eq!(
            Scene::try_new(root.clone()),
            Err(SceneError::DuplicateNode(id(1)))
        );
        let mut scene = Scene::new(root);
        let before = scene.clone();
        assert_eq!(
            scene.apply_batch(&[]),
            Err(SceneError::DuplicateNode(id(1)))
        );
        assert_eq!(scene, before);
        assert_eq!(
            scene.apply(ScenePatch::Remove { id: id(2) }),
            Err(SceneError::DuplicateNode(id(1)))
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn descendant_collision_rolls_back_all_staged_changes() {
        let mut scene =
            Scene::try_new(Node::container(id(1)).child(Node::text(id(2), "original"))).unwrap();
        let before = scene.clone();
        let result = scene.apply_batch(&[
            ScenePatch::UpdateText {
                id: id(2),
                text: "staged".into(),
            },
            ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::container(id(3)).child(Node::rect(id(2))),
            },
        ]);
        assert_eq!(result, Err(SceneError::DuplicateNode(id(2))));
        assert_eq!(scene, before);
    }

    #[test]
    fn duplicate_ids_inside_new_subtree_are_rejected() {
        let mut scene = Scene::new(Node::container(id(1)));
        let before = scene.clone();
        assert_eq!(
            scene.apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::container(id(2))
                    .child(Node::rect(id(3)))
                    .child(Node::container(id(4)).child(Node::rect(id(3)))),
            }),
            Err(SceneError::DuplicateNode(id(3)))
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn remove_then_reinsert_and_update_descendant_uses_staged_state() {
        let mut scene = Scene::try_new(Node::container(id(1)).child(Node::rect(id(2)))).unwrap();
        scene
            .apply_batch(&[
                ScenePatch::Remove { id: id(2) },
                ScenePatch::Insert {
                    parent: id(1),
                    index: 0,
                    node: Node::container(id(3)).child(Node::text(id(2), "new")),
                },
                ScenePatch::UpdateText {
                    id: id(2),
                    text: "updated".into(),
                },
            ])
            .unwrap();
        assert_eq!(scene.version(), 2);
        assert_eq!(scene.find(id(2)).unwrap().text_content(), Some("updated"));
        assert_eq!(scene.validate(), Ok(()));
    }

    #[test]
    fn version_exhaustion_rejects_updates_without_changing_scene() {
        let mut scene = Scene::new(Node::container(id(1)));
        scene.version = u64::MAX - 1;
        scene
            .apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::rect(id(2)),
            })
            .unwrap();
        assert_eq!(scene.version(), u64::MAX);

        assert_eq!(
            scene.apply(ScenePatch::Remove { id: id(2) }),
            Err(SceneError::VersionExhausted)
        );
        assert!(scene.contains(id(2)));
        assert_eq!(scene.version(), u64::MAX);

        // Empty batches are no-ops, but still validate the caller's version.
        // 空批保持无操作语义，同时仍校验调用方版本。
        assert_eq!(scene.apply_batch_at(u64::MAX, &[]), Ok(()));
        assert_eq!(
            scene.apply_batch_at(u64::MAX - 1, &[]),
            Err(SceneError::StaleVersion {
                expected: u64::MAX - 1,
                actual: u64::MAX,
            })
        );
    }

    #[test]
    fn set_scroll_is_versioned_patch() {
        let mut scene = Scene::new(Node::container(id(1)));
        scene
            .apply(ScenePatch::SetScroll {
                id: id(1),
                offset: Point::new(0.0, 24.0),
            })
            .unwrap();
        assert_eq!(scene.find(id(1)).unwrap().scroll, Point::new(0.0, 24.0));
        assert_eq!(scene.version(), 2);
    }

    #[test]
    fn set_canvas_updates_canvas_nodes_only() {
        let mut scene = Scene::new(Node::container(id(1)));
        scene
            .apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::canvas(id(2), Canvas::new()),
            })
            .unwrap();
        let mut canvas = Canvas::new();
        canvas.fill_rect(
            crate::geometry::Rect::new(0.0, 0.0, 4.0, 4.0),
            crate::style::Color::RED,
        );
        scene
            .apply(ScenePatch::SetCanvas {
                id: id(2),
                canvas: canvas.clone(),
            })
            .unwrap();
        assert_eq!(scene.find(id(2)).unwrap().canvas_content(), Some(&canvas));
        assert_eq!(
            scene.apply(ScenePatch::SetCanvas {
                id: id(1),
                canvas: Canvas::new()
            }),
            Err(SceneError::NotCanvas(id(1)))
        );
    }

    #[test]
    fn stale_batch_is_rejected() {
        let mut scene = Scene::new(Node::container(id(1)));
        scene
            .apply(ScenePatch::Insert {
                parent: id(1),
                index: 0,
                node: Node::rect(id(2)),
            })
            .unwrap();
        assert_eq!(
            scene.apply_batch_at(1, &[ScenePatch::Remove { id: id(2) }]),
            Err(SceneError::StaleVersion {
                expected: 1,
                actual: 2
            })
        );
        assert!(scene.contains(id(2)));
        scene
            .apply_batch_at(2, &[ScenePatch::Remove { id: id(2) }])
            .unwrap();
        assert!(!scene.contains(id(2)));
        assert_eq!(scene.version(), 3);
    }
}

#[cfg(test)]
mod semantic_patch_tests {
    use super::*;
    #[test]
    fn semantic_frame_is_versioned_and_rejected_batch_preserves_previous_state() {
        let mut scene=Scene::new(Node::text(NodeId(1),"Caption"));let before=scene.version();
        let semantics=Semantics::default().label("Accessible label").disabled(true);
        scene.apply_batch_at(before,&[ScenePatch::UpdateSemantics {id:NodeId(1),semantics:semantics.clone()}]).unwrap();
        assert_eq!(scene.find(NodeId(1)).unwrap().semantics,semantics);assert_eq!(scene.version(),before+1);
        let version=scene.version();
        let rejected=[ScenePatch::UpdateSemantics {id:NodeId(1),semantics:Semantics::default()},ScenePatch::Remove {id:NodeId(99)}];
        assert!(scene.apply_batch_at(version,&rejected).is_err());assert_eq!(scene.version(),version);
        assert_eq!(scene.find(NodeId(1)).unwrap().semantics,semantics);
    }
}
