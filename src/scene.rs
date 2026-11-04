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
// BEGIN VERIFIED SCENE NODE KIND TYPE
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Container,
    Text(String),
    Rect,
    Canvas(Canvas),
}
// END VERIFIED SCENE NODE KIND TYPE

/// One node in the instance scene.
// BEGIN VERIFIED SCENE NODE TYPE
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
// END VERIFIED SCENE NODE TYPE

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
// BEGIN VERIFIED SCENE STATE TYPE
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub root: Node,
    version: u64,
}
// END VERIFIED SCENE STATE TYPE

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
        commit_scene_batch(
            self,
            expected_version,
            patches.is_empty(),
            Self::validate,
            |active: &Scene| stage_scene_batch(active.clone(), patches, Self::apply_detached),
        )
    }

    /// A plain function with no receiver, so staging cannot reach the active scene.
    /// 无接收者的普通函数，暂存阶段无法触及活动场景。
    fn apply_detached(mut staged: Scene, patch: &ScenePatch) -> Result<Scene, SceneError> {
        staged.apply_one(patch.clone())?;
        Ok(staged)
    }

    fn apply_one(&mut self, patch: ScenePatch) -> Result<(), SceneError> {
        apply_checked_patch(&mut self.root, patch, &mut BTreeSet::new())
    }
}

/// Incremental scene operation.
// BEGIN VERIFIED SCENE PATCH TYPE
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
// END VERIFIED SCENE PATCH TYPE

/// Scene patch failures.
// BEGIN VERIFIED SCENE ERROR TYPE
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
// END VERIFIED SCENE ERROR TYPE

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

// BEGIN VERIFIED SCENE BATCH STAGE KERNEL
/// Folds the patches over a detached value in order, each exactly once, stopping at the first
/// failure. The value moves through the fold, so a failure leaves nothing to undo or to reuse.
/// 按序把补丁折叠到独立值上，每个恰好一次，遇首个失败即停；值随折叠移动，失败后无可撤销也无可复用。
fn stage_scene_batch<S, P, E>(
    detached: S,
    patches: &[P],
    apply: impl Fn(S, &P) -> Result<S, E>,
) -> Result<S, E> {
    let mut staged = detached;
    let mut index = 0usize;
    while index < patches.len() {
        match apply(staged, &patches[index]) {
            Ok(next) => staged = next,
            Err(error) => return Err(error),
        }
        index += 1;
    }
    Ok(staged)
}
// END VERIFIED SCENE BATCH STAGE KERNEL

// BEGIN VERIFIED SCENE BATCH COMMIT KERNEL
/// Decides the version, checks, stages and commits. Both closures only read the active scene, so
/// the single write is the whole replacement at the end, carrying the decided version.
/// 判定版本、检查、暂存、提交。两个闭包只能读活动场景，唯一的写入是末尾的整体替换，版本取判定结果。
fn commit_scene_batch(
    scene: &mut Scene,
    expected_version: u64,
    empty: bool,
    validate: impl FnOnce(&Scene) -> Result<(), SceneError>,
    stage: impl FnOnce(&Scene) -> Result<Scene, SceneError>,
) -> Result<(), SceneError> {
    let next_version = match decide_scene_version(scene.version, expected_version, empty) {
        VersionDecision::Stale => {
            return Err(SceneError::StaleVersion {
                expected: expected_version,
                actual: scene.version,
            })
        }
        VersionDecision::Unchanged => return validate(&*scene),
        VersionDecision::Exhausted => return Err(SceneError::VersionExhausted),
        VersionDecision::Advance(version) => version,
    };
    let checked = validate(&*scene);
    match checked {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let staged = stage(&*scene);
    match staged {
        Ok(staged) => {
            let mut next = staged;
            next.version = next_version;
            *scene = next;
            Ok(())
        }
        Err(error) => Err(error),
    }
}
// END VERIFIED SCENE BATCH COMMIT KERNEL

// BEGIN VERIFIED SCENE IDENTITY SET INSTANCE
// Extracted so the harnesses run the real set; that it obeys the contract is tested, not proved.
// 提取出来让验证运行真实集合；它是否满足契约只经测试，未证明。
impl IdentitySet for BTreeSet<NodeId> {
    fn admit(&mut self, id: NodeId) -> bool {
        self.insert(id)
    }
}
// END VERIFIED SCENE IDENTITY SET INSTANCE

// BEGIN VERIFIED SCENE UNIQUE IDS KERNEL
/// What the identity traversal needs from its set: records an identity and reports whether it was new.
/// 身份遍历对集合的全部要求：记录一个身份，并报告它此前是否未出现。
trait IdentitySet {
    fn admit(&mut self, id: NodeId) -> bool;
}

/// Traverses iteratively so identity validation itself does not recurse.
/// 迭代遍历，身份校验本身不递归；不代表后续布局/克隆已具备深度资源限制。
fn collect_unique_ids<T: IdentitySet>(root: &Node, ids: &mut T) -> Result<(), SceneError> {
    let mut pending = Vec::new();
    pending.push(root);
    loop {
        let node = match pending.pop() {
            Some(node) => node,
            None => return Ok(()),
        };
        if !ids.admit(node.id) {
            return Err(SceneError::DuplicateNode(node.id));
        }
        let mut index = node.children.len();
        while index > 0 {
            index -= 1;
            pending.push(&node.children[index]);
        }
    }
}

/// The identity check for an insertion: the tree first, then the inserted subtree, in one set.
/// 插入的身份检查：先查树，再查待插入子树，共用一个集合。
fn check_insert_identities<T: IdentitySet>(
    root: &Node,
    inserted: &Node,
    ids: &mut T,
) -> Result<(), SceneError> {
    match collect_unique_ids(root, ids) {
        Ok(()) => collect_unique_ids(inserted, ids),
        Err(error) => Err(error),
    }
}
// END VERIFIED SCENE UNIQUE IDS KERNEL

// BEGIN VERIFIED SCENE CHECKED PATCH KERNEL
/// Validates all descendants of an insertion before any follow-up patch can observe ambiguous
/// identities, then applies the patch. The set starts empty and is local to staging.
/// 插入前检查全部后代，避免后续补丁观察到歧义身份，再应用补丁；集合从空开始，仅在暂存阶段使用。
fn apply_checked_patch<T: IdentitySet>(
    root: &mut Node,
    patch: ScenePatch,
    ids: &mut T,
) -> Result<(), SceneError> {
    if let ScenePatch::Insert { node, .. } = &patch {
        match check_insert_identities(&*root, node, ids) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
    }
    apply_tree_patch(root, patch)
}
// END VERIFIED SCENE CHECKED PATCH KERNEL

// BEGIN VERIFIED SCENE PATCH KERNEL
/// Applies one patch to the tree. The tree is the only state reached; a failed patch may be left
/// half-way by nothing: each branch writes only after its checks pass.
/// 把一个补丁应用到树。只触及树本身；每个分支都先通过检查再写入。
fn apply_tree_patch(root: &mut Node, patch: ScenePatch) -> Result<(), SceneError> {
    match patch {
        ScenePatch::Insert {
            parent,
            index,
            node,
        } => match find_node_mut(root, parent) {
            Some(parent_node) => {
                let len = parent_node.children.len();
                let index = if index < len { index } else { len };
                parent_node.children.insert(index, node);
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(parent)),
        },
        ScenePatch::UpdateSemantics { id, semantics } => match find_node_mut(root, id) {
            Some(node) => {
                node.semantics = semantics;
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::UpdateStyle { id, style } => match find_node_mut(root, id) {
            Some(node) => {
                node.style = style;
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::SetScroll { id, offset } => match find_node_mut(root, id) {
            Some(node) => {
                node.scroll = offset;
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::UpdateText { id, text } => match find_node_mut(root, id) {
            Some(node) => match &mut node.kind {
                NodeKind::Text(existing) => {
                    *existing = text;
                    Ok(())
                }
                _ => Err(SceneError::NotText(id)),
            },
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::SetCanvas { id, canvas } => match find_node_mut(root, id) {
            Some(node) => match &mut node.kind {
                NodeKind::Canvas(existing) => {
                    *existing = canvas;
                    Ok(())
                }
                _ => Err(SceneError::NotCanvas(id)),
            },
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::Remove { id } => {
            if root.id.0 == id.0 {
                return Err(SceneError::CannotRemoveRoot);
            }
            if remove_node(root, id) {
                Ok(())
            } else {
                Err(SceneError::NodeNotFound(id))
            }
        }
    }
}
// END VERIFIED SCENE PATCH KERNEL

// BEGIN VERIFIED SCENE NODE FIND KERNEL
/// Finds the first node with the identity in preorder, the node itself before its children.
/// 按前序查找首个身份匹配的节点，节点本身先于其子节点。
fn find_node(node: &Node, id: NodeId) -> Option<&Node> {
    if node.id.0 == id.0 {
        return Some(node);
    }
    let mut index = 0usize;
    while index < node.children.len() {
        if let Some(found) = find_node(&node.children[index], id) {
            return Some(found);
        }
        index += 1;
    }
    None
}

/// Lends the node `find_node` finds. Each level asks the read path which child holds the identity,
/// because a mutable borrow cannot be retried after a miss; a level rescans, so the worst case is
/// nodes times depth. Nothing is written here: the only change to the tree is what the caller
/// does through the returned reference.
/// 借出 `find_node` 找到的节点。每层先用只读路径问清哪个子节点含该身份，因为可变借用在未命中后无法重试；
/// 每层都重新扫描，最坏为节点数乘深度。这里不写任何东西：树的唯一变化是调用者经返回引用所做的修改。
fn find_node_mut(node: &mut Node, id: NodeId) -> Option<&mut Node> {
    if node.id.0 == id.0 {
        return Some(node);
    }
    let mut index = 0usize;
    while index < node.children.len() {
        if find_node(&node.children[index], id).is_some() {
            return find_node_mut(&mut node.children[index], id);
        }
        index += 1;
    }
    None
}
// END VERIFIED SCENE NODE FIND KERNEL

// BEGIN VERIFIED SCENE NODE REMOVE KERNEL
/// Removes the first direct child with the identity; failing that, removes it from the first child
/// subtree that holds it, leaving every other node alone. A miss changes nothing.
/// 优先移除首个身份匹配的直接子节点；否则只在首个含该身份的子树中移除，其余节点不动；未命中不做任何改变。
fn remove_node(parent: &mut Node, id: NodeId) -> bool {
    let mut index = 0usize;
    while index < parent.children.len() {
        if parent.children[index].id.0 == id.0 {
            parent.children.remove(index);
            return true;
        }
        index += 1;
    }
    index = 0;
    while index < parent.children.len() {
        if remove_node(&mut parent.children[index], id) {
            return true;
        }
        index += 1;
    }
    false
}
// END VERIFIED SCENE NODE REMOVE KERNEL

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u64) -> NodeId {
        NodeId(value)
    }

    #[test]
    fn checked_patch_refuses_repeats_before_writing_and_keeps_identities_unique() {
        fn ids(node: &Node, out: &mut Vec<u64>) {
            out.push(node.id.0);
            node.children.iter().for_each(|child| ids(child, out));
        }
        let listed = |scene: &Scene| {
            let mut out = Vec::new();
            ids(&scene.root, &mut out);
            out
        };
        let leaf = |id: u64| Node::container(NodeId(id));
        let base = || Scene::new(leaf(1).child(leaf(2).child(leaf(3))).child(leaf(4)));

        // A repeat inside the tree, against the tree, and inside the subtree are all refused unwritten.
        // 树内重复、与树重复、子树内重复都被拒绝且不写入。
        for (scene, node, repeat) in [
            (Scene::new(leaf(1).child(leaf(1))), leaf(9), 1),
            (base(), leaf(5).child(leaf(3)), 3),
            (base(), leaf(5).child(leaf(5)), 5),
        ] {
            let mut staged = scene.clone();
            assert_eq!(
                staged.apply_one(ScenePatch::Insert {
                    parent: NodeId(1),
                    index: 0,
                    node,
                }),
                Err(SceneError::DuplicateNode(NodeId(repeat)))
            );
            assert_eq!(staged, scene);
        }
        // The identity check comes first: an absent parent reports the repeat, not the miss.
        // 身份检查在先：父节点不存在时仍报重复，而不是未找到。
        let mut staged = base();
        assert_eq!(
            staged.apply_one(ScenePatch::Insert {
                parent: NodeId(99),
                index: 0,
                node: leaf(3),
            }),
            Err(SceneError::DuplicateNode(NodeId(3)))
        );
        assert_eq!(
            staged.apply_one(ScenePatch::Insert {
                parent: NodeId(99),
                index: 0,
                node: leaf(5),
            }),
            Err(SceneError::NodeNotFound(NodeId(99)))
        );
        assert_eq!(staged, base());

        let mut staged = base();
        staged
            .apply_one(ScenePatch::Insert {
                parent: NodeId(2),
                index: 7,
                node: leaf(5).child(leaf(6)),
            })
            .unwrap();
        assert_eq!(listed(&staged), [1, 2, 3, 5, 6, 4]);
        staged
            .apply_one(ScenePatch::Remove { id: NodeId(2) })
            .unwrap();
        assert_eq!(listed(&staged), [1, 4]);
        // Other patches are not checked, so an already repeating tree still takes them.
        // 其他补丁不做检查，已有重复的树仍可接受。
        let mut repeating = Scene::new(leaf(1).child(leaf(1)));
        assert_eq!(
            repeating.apply_one(ScenePatch::SetScroll {
                id: NodeId(1),
                offset: Point::new(1.0, 2.0),
            }),
            Ok(())
        );
    }

    #[test]
    fn batch_commit_writes_only_the_staged_tree_with_the_decided_version() {
        let tree = |id: u64| Node::container(NodeId(id));
        let scene = |version: u64| Scene {
            root: tree(1),
            version,
        };
        let staged = |id: u64| {
            move |_: &Scene| {
                Ok(Scene {
                    root: Node::container(NodeId(id)),
                    version: 77,
                })
            }
        };
        let never_check = |_: &Scene| -> Result<(), SceneError> { panic!("must not check") };
        let never_stage = |_: &Scene| -> Result<Scene, SceneError> { panic!("must not stage") };
        let duplicate = SceneError::DuplicateNode(NodeId(9));

        let mut active = scene(5);
        assert_eq!(
            commit_scene_batch(&mut active, 4, false, never_check, never_stage),
            Err(SceneError::StaleVersion {
                expected: 4,
                actual: 5
            })
        );
        assert_eq!(active, scene(5));
        // Stale wins over an empty batch and over exhaustion.
        // 陈旧优先于空批和耗尽。
        assert_eq!(
            commit_scene_batch(&mut active, 4, true, never_check, never_stage),
            Err(SceneError::StaleVersion {
                expected: 4,
                actual: 5
            })
        );
        let mut full = scene(u64::MAX);
        assert_eq!(
            commit_scene_batch(&mut full, 5, false, never_check, never_stage),
            Err(SceneError::StaleVersion {
                expected: 5,
                actual: u64::MAX
            })
        );
        assert_eq!(
            commit_scene_batch(&mut full, u64::MAX, false, never_check, never_stage),
            Err(SceneError::VersionExhausted)
        );
        assert_eq!(full, scene(u64::MAX));
        // An empty batch at the last version still only checks.
        // 空批即使在最后一个版本也只做检查。
        assert_eq!(
            commit_scene_batch(&mut full, u64::MAX, true, |_| Ok(()), never_stage),
            Ok(())
        );
        assert_eq!(full, scene(u64::MAX));

        assert_eq!(
            commit_scene_batch(
                &mut active,
                5,
                true,
                |_| Err(duplicate.clone()),
                never_stage
            ),
            Err(duplicate.clone())
        );
        assert_eq!(active, scene(5));
        assert_eq!(
            commit_scene_batch(&mut active, 5, true, |_| Ok(()), never_stage),
            Ok(())
        );
        assert_eq!(active, scene(5));
        assert_eq!(
            commit_scene_batch(
                &mut active,
                5,
                false,
                |_| Err(duplicate.clone()),
                never_stage
            ),
            Err(duplicate.clone())
        );
        assert_eq!(active, scene(5));
        assert_eq!(
            commit_scene_batch(
                &mut active,
                5,
                false,
                |_| Ok(()),
                |_| { Err(SceneError::NodeNotFound(NodeId(3))) }
            ),
            Err(SceneError::NodeNotFound(NodeId(3)))
        );
        assert_eq!(active, scene(5));

        // The staged version is overwritten, the staged tree taken whole.
        // 暂存版本被覆盖，暂存树被整体采用。
        assert_eq!(
            commit_scene_batch(&mut active, 5, false, |_| Ok(()), staged(2)),
            Ok(())
        );
        assert_eq!(active.version(), 6);
        assert_eq!(active.root, tree(2));
    }

    #[test]
    fn identity_checks_report_the_first_repeat_in_preorder() {
        let leaf = |id: u64, children: Vec<Node>| {
            let mut node = Node::new(NodeId(id), NodeKind::Rect);
            node.children = children;
            node
        };
        // Preorder 1 2 3 4: the repeat of 3 at position 4 comes first only if children go in order.
        // 前序 1 2 3 4：只有子节点按序遍历，位置靠前的重复才会先被报告。
        let distinct = leaf(1, vec![leaf(2, vec![leaf(3, vec![])]), leaf(4, vec![])]);
        let mut ids = BTreeSet::new();
        assert_eq!(collect_unique_ids(&distinct, &mut ids), Ok(()));
        assert_eq!(
            ids.iter().map(|id| id.0).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );

        let repeats = leaf(
            1,
            vec![
                leaf(2, vec![leaf(5, vec![])]),
                leaf(5, vec![leaf(2, vec![])]),
            ],
        );
        let mut ids = BTreeSet::new();
        assert_eq!(
            collect_unique_ids(&repeats, &mut ids),
            Err(SceneError::DuplicateNode(NodeId(5)))
        );
        assert_eq!(ids.iter().map(|id| id.0).collect::<Vec<_>>(), vec![1, 2, 5]);

        let root_repeated = leaf(7, vec![leaf(7, vec![])]);
        assert_eq!(
            collect_unique_ids(&root_repeated, &mut BTreeSet::new()),
            Err(SceneError::DuplicateNode(NodeId(7)))
        );

        // The tree is checked first, so its repeat wins over one between tree and subtree.
        // 先查树，所以树内重复优先于树与子树之间的重复。
        let inserted = leaf(1, vec![leaf(9, vec![])]);
        assert_eq!(
            check_insert_identities(&repeats, &inserted, &mut BTreeSet::new()),
            Err(SceneError::DuplicateNode(NodeId(5)))
        );
        assert_eq!(
            check_insert_identities(&distinct, &inserted, &mut BTreeSet::new()),
            Err(SceneError::DuplicateNode(NodeId(1)))
        );
        let self_repeating = leaf(8, vec![leaf(8, vec![])]);
        assert_eq!(
            check_insert_identities(&distinct, &self_repeating, &mut BTreeSet::new()),
            Err(SceneError::DuplicateNode(NodeId(8)))
        );
        let mut ids = BTreeSet::new();
        assert_eq!(
            check_insert_identities(&distinct, &leaf(8, vec![leaf(9, vec![])]), &mut ids),
            Ok(())
        );
        assert_eq!(ids.len(), 6);

        // A refused insertion reaches the patch before anything is written.
        // 被拒绝的插入在任何写入之前返回。
        let mut scene = Scene::new(distinct.clone());
        let version = scene.version();
        assert_eq!(
            scene.apply(ScenePatch::Insert {
                parent: NodeId(1),
                index: 0,
                node: leaf(4, vec![])
            }),
            Err(SceneError::DuplicateNode(NodeId(4)))
        );
        assert_eq!(scene.root, distinct);
        assert_eq!(scene.version(), version);
    }

    #[test]
    fn apply_tree_patch_writes_only_what_each_patch_names() {
        let build = || {
            Node::container(id(1)).with_children(vec![
                Node::text(id(2), "first"),
                Node::canvas(id(3), Canvas::default()),
                Node::rect(id(4)),
            ])
        };
        let apply = |patch| {
            let mut tree = build();
            let result = apply_tree_patch(&mut tree, patch);
            (tree, result)
        };
        let missing = SceneError::NodeNotFound(id(9));

        // Each kind of patch misses without touching the tree. 每种补丁未命中都不改树。
        for patch in [
            ScenePatch::Insert {
                parent: id(9),
                index: 0,
                node: Node::rect(id(5)),
            },
            ScenePatch::UpdateStyle {
                id: id(9),
                style: Style::default(),
            },
            ScenePatch::UpdateSemantics {
                id: id(9),
                semantics: Semantics::default(),
            },
            ScenePatch::UpdateText {
                id: id(9),
                text: "x".into(),
            },
            ScenePatch::SetCanvas {
                id: id(9),
                canvas: Canvas::default(),
            },
            ScenePatch::SetScroll {
                id: id(9),
                offset: Point::new(1.0, 2.0),
            },
            ScenePatch::Remove { id: id(9) },
        ] {
            assert_eq!(apply(patch), (build(), Err(missing.clone())));
        }

        // An index past the end lands last; an exact one lands there. 越界下标落到末尾；精确下标落在原位。
        let (tree, result) = apply(ScenePatch::Insert {
            parent: id(1),
            index: 99,
            node: Node::rect(id(5)),
        });
        assert_eq!(result, Ok(()));
        let order: Vec<u64> = tree.children.iter().map(|node| node.id.0).collect();
        assert_eq!(order, [2, 3, 4, 5]);
        let (tree, _) = apply(ScenePatch::Insert {
            parent: id(1),
            index: 1,
            node: Node::rect(id(5)),
        });
        let order: Vec<u64> = tree.children.iter().map(|node| node.id.0).collect();
        assert_eq!(order, [2, 5, 3, 4]);

        let (tree, result) = apply(ScenePatch::SetScroll {
            id: id(4),
            offset: Point::new(1.0, 2.0),
        });
        let mut expected = build();
        expected.children[2].scroll = Point::new(1.0, 2.0);
        assert_eq!((tree, result), (expected, Ok(())));

        // Text and canvas writes need the matching kind. 文本与画布写入须种类匹配。
        let (tree, result) = apply(ScenePatch::UpdateText {
            id: id(2),
            text: "second".into(),
        });
        let mut expected = build();
        expected.children[0].kind = NodeKind::Text("second".into());
        assert_eq!((tree, result), (expected, Ok(())));
        assert_eq!(
            apply(ScenePatch::UpdateText {
                id: id(3),
                text: "x".into(),
            }),
            (build(), Err(SceneError::NotText(id(3))))
        );
        assert_eq!(
            apply(ScenePatch::SetCanvas {
                id: id(2),
                canvas: Canvas::default(),
            }),
            (build(), Err(SceneError::NotCanvas(id(2))))
        );

        // The root is refused; a child is removed. 根被拒；子节点被移除。
        assert_eq!(
            apply(ScenePatch::Remove { id: id(1) }),
            (build(), Err(SceneError::CannotRemoveRoot))
        );
        let (tree, result) = apply(ScenePatch::Remove { id: id(2) });
        let mut expected = build();
        expected.children.remove(0);
        assert_eq!((tree, result), (expected, Ok(())));
    }

    #[test]
    fn find_node_paths_agree_on_the_first_match_in_preorder() {
        // Identity 9 is the second child's descendant and also a later direct child; the root's
        // own identity repeats below it. 身份 9 在第二个子节点之下，之后又是直接子节点；根身份在其下重复。
        let build = || {
            Node::container(id(1)).with_children(vec![
                Node::rect(id(2)),
                Node::container(id(3)).with_children(vec![
                    Node::rect(id(4)),
                    Node::text(id(9), "nested"),
                    Node::rect(id(1)),
                ]),
                Node::text(id(9), "direct"),
                Node::container(id(5)).child(Node::rect(id(6))),
            ])
        };
        let tree = build();
        assert_eq!(find_node(&tree, id(1)).map(|n| n.children.len()), Some(4));
        assert_eq!(
            find_node(&tree, id(9)).map(|n| &n.kind),
            Some(&NodeKind::Text("nested".into()))
        );
        assert_eq!(find_node(&tree, id(6)).map(|n| n.id), Some(id(6)));
        assert!(find_node(&tree, id(7)).is_none());

        let mut written = build();
        let target = find_node_mut(&mut written, id(9)).unwrap();
        assert_eq!(target.kind, NodeKind::Text("nested".into()));
        target.scroll = Point::new(3.0, 4.0);
        let mut expected = build();
        expected.children[1].children[1].scroll = Point::new(3.0, 4.0);
        assert_eq!(written, expected);

        // A miss leaves the tree alone. 未命中不改树。
        assert!(find_node_mut(&mut written, id(7)).is_none());
        assert_eq!(written, expected);
        // The root wins over its repeated identity. 根优先于其下重复的身份。
        assert_eq!(
            find_node_mut(&mut written, id(1)).unwrap().children.len(),
            4
        );
    }

    #[test]
    fn remove_node_prefers_a_direct_child_and_touches_one_subtree() {
        // Identity 9 sits in a nested subtree first, then as a direct child, then nested again.
        // 身份 9 先在嵌套子树，再是直接子节点，之后又在嵌套子树；这里绕过 Scene 的唯一性检查。
        let tree = |children: Vec<Node>| Node::container(id(1)).with_children(children);
        let nested = |at: u64| Node::container(id(at)).child(Node::rect(id(9)));
        let mut direct = tree(vec![
            nested(2),
            Node::text(id(9), "gone"),
            Node::rect(id(3)),
            nested(4),
        ]);
        assert!(remove_node(&mut direct, id(9)));
        assert_eq!(direct, tree(vec![nested(2), Node::rect(id(3)), nested(4)]));

        let mut deep = tree(vec![Node::rect(id(3)), nested(2), nested(4)]);
        assert!(remove_node(&mut deep, id(9)));
        assert_eq!(
            deep,
            tree(vec![Node::rect(id(3)), Node::container(id(2)), nested(4)])
        );

        let before = deep.clone();
        assert!(remove_node(&mut deep, id(9)));
        assert_eq!(deep.children[2], Node::container(id(4)));
        let after = deep.clone();
        assert!(!remove_node(&mut deep, id(9)));
        assert_eq!(deep, after);
        assert_ne!(before, after);
        // The root's own identity is not its descendant. 根自身不是自己的后代。
        assert!(!remove_node(&mut deep, id(1)));
        assert_eq!(deep, after);
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
        let mut scene = Scene::new(Node::text(NodeId(1), "Caption"));
        let before = scene.version();
        let semantics = Semantics::default()
            .label("Accessible label")
            .disabled(true);
        scene
            .apply_batch_at(
                before,
                &[ScenePatch::UpdateSemantics {
                    id: NodeId(1),
                    semantics: semantics.clone(),
                }],
            )
            .unwrap();
        assert_eq!(scene.find(NodeId(1)).unwrap().semantics, semantics);
        assert_eq!(scene.version(), before + 1);
        let version = scene.version();
        let rejected = [
            ScenePatch::UpdateSemantics {
                id: NodeId(1),
                semantics: Semantics::default(),
            },
            ScenePatch::Remove { id: NodeId(99) },
        ];
        assert!(scene.apply_batch_at(version, &rejected).is_err());
        assert_eq!(scene.version(), version);
        assert_eq!(scene.find(NodeId(1)).unwrap().semantics, semantics);
    }
}
