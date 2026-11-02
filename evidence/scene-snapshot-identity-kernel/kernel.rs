use vstd::prelude::*;
verus! {
impl<T> ViewSlot<T> { pub closed spec fn owned_value(&self) -> Option<T> { self.value } }



use std::collections::{BTreeMap,BTreeSet}; pub mod blitz_dom {#[verifier::external_body] pub struct NodeId {} #[verifier::external_body] pub struct FontContext {}} #[verifier::external_body] pub struct BaseDocument {} #[verifier::external_body] pub struct Scene {}

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
    source:Scene,
    font_ctx:blitz_dom::FontContext,
    bold_aliases:BTreeSet<u64>,
    document:ViewSlot<BaseDocument>,
    owner:u64,
    scene_version:u64,
    node_count:usize,
    parents:BTreeMap<NodeId,Option<NodeId>>,
    children:BTreeMap<NodeId,Vec<NodeId>>,
    displayed:BTreeMap<NodeId,bool>,
    semantics:BTreeMap<NodeId,Semantics>,
    nodes:BTreeMap<NodeId,(blitz_dom::NodeId,Option<blitz_dom::NodeId>)>,
    frame_dirty:bool,
    frame_cache:Option<(u32,u32,Vec<u8>)>,
    families:BTreeSet<String>,
    last_pointer:Option<(f32,f32)>,
}

impl NativeScene {
    pub(crate) fn snapshot_identity(&self,caller:u64)->(result:Option<(u64,usize)>)
    ensures result == (if caller==self.owner && self.document.owned_value().is_some() {Some((self.scene_version,self.node_count))} else {None}),
    {
        if caller!=self.owner || !self.document.is_open() {return None;}
        Some((self.scene_version,self.node_count))
    }
}

}
