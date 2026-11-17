use vstd::prelude::*;
verus! {
impl<T> ViewSlot<T> { pub closed spec fn owned_value(&self) -> Option<T> { self.value } }



use std::collections::{BTreeMap,BTreeSet}; pub mod blitz_dom {#[verifier::external_body] pub struct NodeId {} #[verifier::external_body] pub struct FontContext {}} #[verifier::external_body] pub struct BaseDocument {} #[verifier::external_body] pub struct Scene {} pub mod jasper_ui_engine {pub mod motion {#[verifier::external_body] pub struct MotionState {}}} #[verifier::external_body] pub struct MaterialState {} pub mod scene_edit {#[verifier::external_body] pub struct EditState {}}

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
    pub(crate) fn snapshot_identity(&self,caller:u64)->(result:Option<(u64,usize)>)
    ensures result == (if caller==self.owner && self.document.owned_value().is_some() {Some((self.scene_version,self.node_count))} else {None}),
    {
        if caller!=self.owner {
            return None;
        }
        Some((self.scene_version, self.node_count))
    }
}

}
