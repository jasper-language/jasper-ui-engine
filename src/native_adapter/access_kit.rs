//! AccessKit bridge for the host-neutral accessibility tree: snapshots and incremental updates
//! become AccessKit TreeUpdate values, and AccessKit action requests are admitted against the
//! actions the tree reports before they reach focus/activate. No platform adapter, window or
//! event loop is created here; the host owns the AccessKit platform adapter.
//! 宿主无关无障碍树的 AccessKit 桥：快照与增量更新转换为 AccessKit TreeUpdate，AccessKit 动作请求
//! 先按树中报告的动作准入再交给 focus/activate。本模块不创建平台适配器、窗口或事件循环；
//! AccessKit 平台适配器归宿主。
use accesskit::{Action,ActionRequest,Affine,Node,NodeId as KitId,Rect as KitRect,Role as KitRole,TreeId,TreeInfo,TreeUpdate};
use jasper_ui_engine::{scene::NodeId,semantics::Role};
use crate::access_tree::{AccessibilityNode,AccessibilityTree,AccessibilityUpdate};
use crate::native::host::SceneHostView;
use crate::native::{SceneActivationEvent,SceneFocusEvent};

// BEGIN VERIFIED ACCESS REQUEST TYPE
/// What an AccessKit action request asks of a node, as far as this engine is concerned.
/// 就本引擎而言，AccessKit 动作请求要求节点做什么。
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum AccessRequest {
    /// AccessKit Focus.
    Focus,
    /// AccessKit Click (default action).
    /// AccessKit Click（默认动作）。
    Activate,
    /// Any other AccessKit action; never admitted.
    /// 其他任何 AccessKit 动作；一律不准入。
    Unsupported,
}
// END VERIFIED ACCESS REQUEST TYPE

// BEGIN VERIFIED ACCESS REQUEST KERNEL
/// A request is admitted exactly when the node offers the matching action (`focus`/`activate`
/// as reported in the accessibility tree); unsupported requests never are.
/// 请求当且仅当节点提供对应动作（无障碍树中报告的 `focus`/`activate`）时准入；不支持的请求一律拒绝。
pub fn access_request_admits(request: AccessRequest, focus: bool, activate: bool) -> bool {
    match request {
        AccessRequest::Focus => focus,
        AccessRequest::Activate => activate,
        AccessRequest::Unsupported => false,
    }
}
// END VERIFIED ACCESS REQUEST KERNEL

/// AccessKit action to the engine request it maps to.
/// AccessKit 动作到对应引擎请求。
pub fn access_request(action:Action)->AccessRequest {
    match action {
        Action::Focus=>AccessRequest::Focus,
        Action::Click=>AccessRequest::Activate,
        _=>AccessRequest::Unsupported,
    }
}

/// Framework role to AccessKit role; Unknown stays Unknown.
/// Framework 角色到 AccessKit 角色；Unknown 仍为 Unknown。
pub fn accesskit_role(role:Role)->KitRole {
    match role {
        Role::Container=>KitRole::GenericContainer,
        Role::Text=>KitRole::Label,
        Role::Button=>KitRole::Button,
        Role::TextField=>KitRole::TextInput,
        Role::Checkbox=>KitRole::CheckBox,
        Role::Radio=>KitRole::RadioButton,
        Role::Switch=>KitRole::Switch,
        Role::Slider=>KitRole::Slider,
        Role::Image=>KitRole::Image,
        Role::Link=>KitRole::Link,
        Role::List=>KitRole::List,
        Role::ListItem=>KitRole::ListItem,
        Role::Dialog=>KitRole::Dialog,
        Role::Menu=>KitRole::Menu,
        Role::MenuItem=>KitRole::MenuItem,
        Role::ScrollView=>KitRole::ScrollView,
        Role::Unknown=>KitRole::Unknown,
    }
}

/// One node in AccessKit form. A Text node without a value carries its label as the AccessKit
/// value, since AccessKit reads Label text content from the value. Focus is reported through
/// TreeUpdate::focus, not on the node. `transform` is set only on the root.
/// AccessKit 形式的单个节点。无 value 的 Text 节点把 label 作为 AccessKit value（AccessKit 从 value
/// 读取 Label 的文字内容）。焦点经 TreeUpdate::focus 报告，不在节点上。`transform` 只设在根上。
fn accesskit_node(node:&AccessibilityNode,transform:Option<Affine>)->Node {
    let mut out=Node::new(accesskit_role(node.role));
    if node.role==Role::Text && node.value.is_empty() {
        if !node.label.is_empty() {out.set_value(node.label.clone());}
    } else {
        if !node.label.is_empty() {out.set_label(node.label.clone());}
        if !node.value.is_empty() {out.set_value(node.value.clone());}
    }
    if !node.description.is_empty() {out.set_description(node.description.clone());}
    if node.disabled {out.set_disabled();}
    if let Some(rect)=node.bounds {
        let (x,y)=(rect.origin.x as f64,rect.origin.y as f64);
        out.set_bounds(KitRect {x0:x,y0:y,x1:x+rect.size.width as f64,y1:y+rect.size.height as f64});
    }
    if node.actions.focus {out.add_action(Action::Focus);}
    if node.actions.activate {out.add_action(Action::Click);}
    out.set_children(node.children.iter().map(|id|KitId(id.0)).collect::<Vec<_>>());
    if let Some(transform)=transform {out.set_transform(transform);}
    out
}

fn tree_update(nodes:&[AccessibilityNode],root:Option<NodeId>,focused:Option<NodeId>,transform:Affine)->Option<TreeUpdate> {
    let root=root?;
    let root_transform=(transform!=Affine::IDENTITY).then_some(transform);
    let nodes=nodes.iter().map(|node|(KitId(node.node.0),
        accesskit_node(node,if node.node==root {root_transform} else {None}))).collect();
    let mut tree=TreeInfo::new(KitId(root.0));
    tree.toolkit_name=Some("Jasper UI".into());
    tree.toolkit_version=Some(env!("CARGO_PKG_VERSION").into());
    Some(TreeUpdate {nodes,tree:Some(tree),tree_id:TreeId::ROOT,focus:KitId(focused.unwrap_or(root).0)})
}

impl AccessibilityTree {
    /// Full AccessKit TreeUpdate for this tree (initial update or resynchronisation), in the
    /// root AccessKit tree. `transform` maps view-local logical coordinates to the host
    /// adapter's coordinate space (scale factor, view offset) and is placed on the root.
    /// Focus falls back to the root when no node is focused. None for an empty tree: AccessKit
    /// cannot represent a tree without a root.
    /// 本树的完整 AccessKit TreeUpdate（初始更新或重新同步），位于 AccessKit 根树。`transform` 把视图局部
    /// 逻辑坐标映射到宿主适配器坐标空间（缩放、视图偏移），设在根上。无焦点节点时焦点回落到根。
    /// 空树为 None：AccessKit 无法表示无根的树。
    pub fn to_accesskit(&self,transform:Affine)->Option<TreeUpdate> {
        tree_update(&self.nodes,self.root(),self.focused,transform)
    }
}

impl AccessibilityUpdate {
    /// Incremental AccessKit TreeUpdate: the added or changed nodes in full; removed nodes are
    /// dropped by AccessKit through their changed parents, as AccessKit requires. Tree info is
    /// always included, so a root change needs nothing else. `transform` must equal the one the
    /// mirror was built with; a changed transform needs a full update. None when the newer tree
    /// is empty.
    /// 增量 AccessKit TreeUpdate：新增或变更的节点（完整内容）；按 AccessKit 规则，移除的节点经其变更的
    /// 父节点由 AccessKit 丢弃。总是附带树信息，根变化无需另行处理。`transform` 须与构建镜像时相同；
    /// 变换改变需发完整更新。较新树为空时为 None。
    pub fn to_accesskit(&self,transform:Affine)->Option<TreeUpdate> {
        tree_update(&self.nodes,self.root,self.focused,transform)
    }
}

/// Routes an AccessKit action request: the target must be in the current accessibility tree of
/// the root AccessKit tree and offer the requested action; Focus goes to focus, Click to
/// activate. Requests carrying action data, other actions, other trees, stale versions and
/// non-owners are refused without effect.
/// 路由 AccessKit 动作请求：目标须在 AccessKit 根树对应的当前无障碍树中且提供所请求的动作；Focus 交给
/// focus，Click 交给 activate。携带动作数据、其他动作、其他树、过期版本与非归属方一律拒绝且无副作用。
pub(crate) fn route_action(view:&mut SceneHostView,owner:u64,version:u64,request:&ActionRequest,
    focus:&mut impl FnMut(SceneFocusEvent),activation:&mut impl FnMut(SceneActivationEvent))->bool {
    if request.target_tree!=TreeId::ROOT || request.data.is_some() {return false;}
    let Some(tree)=view.accessibility_tree(owner,version) else {return false};
    let target=NodeId(request.target_node.0);
    let Some(node)=tree.nodes.iter().find(|node|node.node==target) else {return false};
    let kind=access_request(request.action);
    if !access_request_admits(kind,node.actions.focus,node.actions.activate) {return false;}
    match kind {
        AccessRequest::Focus=>view.focus(owner,version,target,focus),
        AccessRequest::Activate=>view.activate(owner,version,target,activation),
        AccessRequest::Unsupported=>false,
    }
}
