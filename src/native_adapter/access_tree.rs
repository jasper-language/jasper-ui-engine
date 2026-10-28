//! Host-neutral accessibility tree for platform bridges (AccessKit, AT-SPI, UIA, the system
//! accessibility service); this module does not call any platform accessibility API.
//! 供平台无障碍桥（AccessKit、AT-SPI、UIA、系统无障碍服务）消费的宿主无关无障碍树；本模块不调用任何平台无障碍 API。
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
