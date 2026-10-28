//! Host-neutral accessibility tree for platform bridges (AccessKit, AT-SPI, UIA, the system
//! accessibility service); this module does not call any platform accessibility API.
//! 供平台无障碍桥（AccessKit、AT-SPI、UIA、系统无障碍服务）消费的宿主无关无障碍树；本模块不调用任何平台无障碍 API。
use std::collections::{HashMap,HashSet};
use jasper_ui_engine::{geometry::Rect,scene::NodeId,semantics::Role};

// BEGIN VERIFIED ACCESS ACTIONS TYPE
/// Actions an assistive technology may request on a node; a disabled node offers none.
/// 辅助技术可对节点请求的动作；禁用节点没有任何动作。
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct AccessActions {
    /// The node accepts Engine::focus.
    /// 节点接受 Engine::focus。
    pub focus: bool,
    /// The node accepts Engine::activate.
    /// 节点接受 Engine::activate。
    pub activate: bool,
}
// END VERIFIED ACCESS ACTIONS TYPE

// BEGIN VERIFIED ACCESS ACTIONS KERNEL
/// Actions of a node already in the tree (so visible): focus for an enabled focusable node,
/// activate for an enabled button; the same rules focus/activate admit.
/// 已在树中（即可见）节点的动作：启用且可聚焦者可聚焦，启用的按钮可激活；与 focus/activate 的准入规则一致。
pub fn access_actions(is_button: bool, focusable: bool, disabled: bool) -> AccessActions {
    AccessActions { focus: focusable && !disabled, activate: is_button && !disabled }
}
// END VERIFIED ACCESS ACTIONS KERNEL

/// One visible source node with Framework semantics, view-local geometry, state and actions.
/// 一个可见源节点：Framework 语义、视图局部几何、状态与动作。
#[derive(Clone,Debug,PartialEq)]
pub struct AccessibilityNode {
    pub node: NodeId,
    pub role: Role,
    pub label: String,
    pub description: String,
    pub value: String,
    pub disabled: bool,
    /// Same as node_bounds (view-local logical coordinates); None where node_bounds is None.
    /// 同 node_bounds（视图局部逻辑坐标）；node_bounds 为 None 时为 None。
    pub bounds: Option<Rect>,
    /// Scene-local focus; the platform combines window/application focus.
    /// 场景内焦点；窗口/应用焦点由平台合并。
    pub focused: bool,
    pub actions: AccessActions,
    /// Visible children in source order.
    /// 源顺序中的可见子节点。
    pub children: Vec<NodeId>,
}

/// Versioned accessibility tree: visible nodes in depth-first source order, root first.
/// Hidden nodes and nodes not displayed are left out together with their subtrees.
/// 带版本的无障碍树：可见节点按源顺序深度优先、根在首位；隐藏或未显示的节点连同其子树一并剔除。
#[derive(Clone,Debug,PartialEq)]
pub struct AccessibilityTree {
    pub scene_version: u64,
    /// Empty when the root itself is hidden or not displayed.
    /// 根节点本身隐藏或未显示时为空。
    pub nodes: Vec<AccessibilityNode>,
    /// The focused node when it is in this tree.
    /// 焦点节点（仅当它在本树中）。
    pub focused: Option<NodeId>,
}

// BEGIN VERIFIED ACCESS CHANGE TYPE
/// How one node differs between two accessibility trees.
/// 同一节点在两份无障碍树之间的差别。
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum AccessChange {
    /// In neither tree.
    /// 两份树中都没有。
    Absent,
    /// Only in the newer tree.
    /// 只在较新的树中。
    Added,
    /// Only in the older tree.
    /// 只在较旧的树中。
    Removed,
    /// In both trees with equal content.
    /// 两份树中都有且内容相同。
    Unchanged,
    /// In both trees with different content.
    /// 两份树中都有但内容不同。
    Changed,
}
// END VERIFIED ACCESS CHANGE TYPE

// BEGIN VERIFIED ACCESS CHANGE KERNEL
/// Classifies a node from its presence in the older and newer tree; `equal` (content equality)
/// only matters when the node is in both.
/// 按节点在旧树与新树中是否存在分类；`equal`（内容相等）只在两树都有该节点时起作用。
pub fn access_change(in_previous: bool, in_next: bool, equal: bool) -> AccessChange {
    if !in_next {
        if in_previous { AccessChange::Removed } else { AccessChange::Absent }
    } else if !in_previous {
        AccessChange::Added
    } else if equal {
        AccessChange::Unchanged
    } else {
        AccessChange::Changed
    }
}
// END VERIFIED ACCESS CHANGE KERNEL

/// Incremental change from one accessibility tree to a newer one of the same view, shaped for
/// bridges that keep a mirror (AccessKit TreeUpdate style): every added or changed node in full,
/// nodes that left the tree, the root and the focus.
/// 同一视图从一份无障碍树到较新一份的增量变化，形如维护镜像的桥（AccessKit TreeUpdate 风格）所需：
/// 新增或变更的节点（完整内容）、离开树的节点、根与焦点。
#[derive(Clone,Debug,PartialEq)]
pub struct AccessibilityUpdate {
    pub from_version: u64,
    pub scene_version: u64,
    /// Added or changed nodes in the newer tree's order. A parent whose visible children changed
    /// is itself changed.
    /// 新增或变更的节点，按较新树的顺序。可见子节点变化的父节点本身即为变更。
    pub nodes: Vec<AccessibilityNode>,
    /// Nodes of the older tree that are not in the newer one, in the older tree's order.
    /// 旧树中有、新树中没有的节点，按旧树顺序。
    pub removed: Vec<NodeId>,
    /// Root of the newer tree; None when it is empty.
    /// 较新树的根；树为空时为 None。
    pub root: Option<NodeId>,
    pub focused: Option<NodeId>,
    pub focus_changed: bool,
}

impl AccessibilityUpdate {
    /// No node, root or focus difference.
    /// 节点、根与焦点均无差别。
    pub fn is_empty(&self)->bool {
        self.nodes.is_empty() && self.removed.is_empty() && !self.focus_changed
    }
}

impl AccessibilityTree {
    /// Root node id; None for an empty tree.
    /// 根节点；空树为 None。
    pub fn root(&self)->Option<NodeId> {self.nodes.first().map(|node|node.node)}

    /// Difference from `previous`, an older or equal-version tree of the same view. None when
    /// `previous` is newer. Trees of different views are not detected.
    /// 相对 `previous`（同一视图较旧或同版本的树）的差。`previous` 更新时为 None；不检测跨视图混用。
    pub fn update_since(&self,previous:&AccessibilityTree)->Option<AccessibilityUpdate> {
        if previous.scene_version>self.scene_version {return None;}
        let old:HashMap<NodeId,&AccessibilityNode>=previous.nodes.iter().map(|node|(node.node,node)).collect();
        let new:HashSet<NodeId>=self.nodes.iter().map(|node|node.node).collect();
        let nodes=self.nodes.iter().filter(|node|{
            let before=old.get(&node.node);
            matches!(access_change(before.is_some(),true,before.is_some_and(|before|*before==*node)),
                AccessChange::Added|AccessChange::Changed)
        }).cloned().collect();
        let removed=previous.nodes.iter().map(|node|node.node)
            .filter(|id|access_change(true,new.contains(id),false)==AccessChange::Removed).collect();
        Some(AccessibilityUpdate {from_version:previous.scene_version,scene_version:self.scene_version,nodes,removed,
            root:self.root(),focused:self.focused,focus_changed:self.focused!=previous.focused})
    }

    /// Applies an update produced against this tree and rebuilds depth-first source order from
    /// the root. None when the update was made against another version, removes an absent
    /// node, or leaves a missing, duplicated or unreachable child or a focus outside the tree.
    /// 应用基于本树产生的更新，并从根重建深度优先源顺序。更新基于其他版本、移除不存在的节点，
    /// 或导致子节点缺失、重复、不可达，或焦点不在树中时为 None。
    pub fn apply(&self,update:&AccessibilityUpdate)->Option<AccessibilityTree> {
        if update.from_version!=self.scene_version || update.scene_version<self.scene_version {return None;}
        let mut nodes:HashMap<NodeId,AccessibilityNode>=self.nodes.iter().map(|node|(node.node,node.clone())).collect();
        for id in &update.removed {nodes.remove(id)?;}
        for node in &update.nodes {nodes.insert(node.node,node.clone());}
        let mut ordered=Vec::with_capacity(nodes.len());
        let mut seen=HashSet::with_capacity(nodes.len());
        let mut pending:Vec<NodeId>=update.root.into_iter().collect();
        while let Some(id)=pending.pop() {
            if !seen.insert(id) {return None;}
            let node=nodes.get(&id)?;
            pending.extend(node.children.iter().rev());
            ordered.push(node.clone());
        }
        if ordered.len()!=nodes.len() {return None;}
        if update.focused.is_some_and(|id|!seen.contains(&id)) {return None;}
        for node in &mut ordered {node.focused=update.focused==Some(node.node);}
        Some(AccessibilityTree {scene_version:update.scene_version,nodes:ordered,focused:update.focused})
    }
}
