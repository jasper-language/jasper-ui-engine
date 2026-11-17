use vstd::prelude::*;
verus! {
impl<T> ViewSlot<T> { pub closed spec fn owned_value(&self) -> Option<T> { self.value } }



use std::collections::{BTreeMap,BTreeSet}; pub mod blitz_dom {#[verifier::external_body] pub struct NodeId {} #[verifier::external_body] pub struct FontContext {}} #[verifier::external_body] pub struct BaseDocument {} #[verifier::external_body] pub struct Scene {} pub mod jasper_ui_engine {pub mod motion {#[verifier::external_body] pub struct MotionState {}}} #[verifier::external_body] pub struct MaterialState {} pub mod scene_edit {#[verifier::external_body] pub struct EditState {}}
pub closed spec fn native_scene_metadata_same(a:&NativeScene,b:&NativeScene)->bool {a.owner==b.owner && a.scene_version==b.scene_version && a.node_count==b.node_count && a.parents@==b.parents@ && a.children@==b.children@ && a.displayed@==b.displayed@ && a.semantics@==b.semantics@ && a.nodes@==b.nodes@}
pub(crate) mod native_scene {pub(crate) use super::NativeScene;}

pub closed spec fn scene_host_identity_model(view:&SceneHostView,owner:u64)->Option<(u64,usize)> {if owner==view.inner.owner && view.inner.document.owned_value().is_some() {Some((view.inner.scene_version,view.inner.node_count))} else {None}}
pub struct ViewSlot<T> {
    value: Option<T>,
}

impl<T> ViewSlot<T> {
    pub fn new(value: T) -> (result: Self)
    ensures result.owned_value() == Some(value),
{
        Self { value: Some(value) }
    }

    /// Retires the owned value before its destructor runs; never reopens the slot.
    /// This is logical ownership retirement, not an external cancellation ACK.
    pub fn close(&mut self) -> (result: bool)
    ensures final(self).owned_value().is_none(), result == old(self).owned_value().is_some(),
{
        self.value.take().is_some()
    }

    pub fn is_open(&self) -> (result: bool)
    ensures result == self.owned_value().is_some(),
{
        self.value.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

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
    /// Section heading; the level is 1..=6 by construction.
    /// 章节标题；级别由构造保证为 1..=6。
    Heading(HeadingLevel),
    /// Advisory status output (implicitly a polite live region).
    /// 状态信息（隐含礼貌型实时区域）。
    Status,
    /// Live region announcing its changes with the given politeness.
    /// 以指定礼貌级别播报变化的实时区域。
    Live(LivePoliteness),
    /// Important, time-sensitive message (implicitly an assertive live region).
    /// 重要且有时效的消息（隐含强制型实时区域）。
    Alert,
    /// Form landmark grouping input controls.
    /// 表单地标，组合输入控件。
    Form,
}

/// Heading level, always within 1..=6; build with `HeadingLevel::new`.
/// 标题级别，恒在 1..=6；经 `HeadingLevel::new` 构造。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingLevel(u8);

/// How assertively a live region's changes are announced.
/// 实时区域变化的播报强度。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LivePoliteness {
    Polite,
    Assertive,
}

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

pub(crate) struct NativeScene {
    source: Scene,
    font_ctx: blitz_dom::FontContext,
    bold_aliases: BTreeSet<u64>,
    document: ViewSlot<BaseDocument>,
    owner: u64,
    scene_version: u64,
    node_count: usize,
    parents: BTreeMap<NodeId, Option<NodeId>>,
    children: BTreeMap<NodeId, Vec<NodeId>>,
    displayed: BTreeMap<NodeId, bool>,
    semantics: BTreeMap<NodeId, Semantics>,
    nodes: BTreeMap<NodeId, (blitz_dom::NodeId, Option<blitz_dom::NodeId>)>,
    frame_dirty: bool,
    frame_cache: Option<(u32, u32, Vec<u8>)>,
    families: BTreeSet<String>,
    last_pointer: Option<(f32, f32)>,
    motion: jasper_ui_engine::motion::MotionState,
    material: MaterialState,
    edit: scene_edit::EditState,
}

impl NativeScene {
    pub(crate) fn close(&mut self,caller:u64)->(result:bool)
 ensures result==(caller==old(self).owner && old(self).document.owned_value().is_some()),
    if caller==old(self).owner {final(self).document.owned_value().is_none()} else {final(self).document.owned_value()==old(self).document.owned_value()},
    native_scene_metadata_same(old(self),final(self)),
 {proof {reveal(native_scene_metadata_same);}
        caller == self.owner && self.document.close()
    }
}

pub struct SceneHostView {inner:native_scene::NativeScene}

impl NativeScene {
    pub(crate) fn snapshot_identity(&self,caller:u64)->(result:Option<(u64,usize)>)
    ensures result == (if caller==self.owner && self.document.owned_value().is_some() {Some((self.scene_version,self.node_count))} else {None}),
    {
        if caller != self.owner || !self.document.is_open() {
            return None;
        }
        Some((self.scene_version, self.node_count))
    }
}

impl SceneHostView {
    pub fn snapshot_identity(&self,owner:u64)->(result:Option<(u64,usize)>)
    ensures result == scene_host_identity_model(self,owner),
    { proof {reveal(scene_host_identity_model);}
        self.inner.snapshot_identity(owner)
    }
}

}
