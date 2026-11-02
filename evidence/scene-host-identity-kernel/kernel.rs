use vstd::prelude::*;
verus! {
impl<T> ViewSlot<T> { pub closed spec fn owned_value(&self) -> Option<T> { self.value } }



use std::collections::{BTreeMap,BTreeSet}; pub mod blitz_dom {#[verifier::external_body] pub struct NodeId {} #[verifier::external_body] pub struct FontContext {}} #[verifier::external_body] pub struct BaseDocument {} #[verifier::external_body] pub struct Scene {}
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
    pub(crate) fn close(&mut self,caller:u64)->(result:bool)
 ensures result==(caller==old(self).owner && old(self).document.owned_value().is_some()),
    if caller==old(self).owner {final(self).document.owned_value().is_none()} else {final(self).document.owned_value()==old(self).document.owned_value()},
    native_scene_metadata_same(old(self),final(self)),
 {proof {reveal(native_scene_metadata_same);}
        caller==self.owner && self.document.close()
    }
}

pub struct SceneHostView {inner:native_scene::NativeScene}

impl NativeScene {
    pub(crate) fn snapshot_identity(&self,caller:u64)->(result:Option<(u64,usize)>)
    ensures result == (if caller==self.owner && self.document.owned_value().is_some() {Some((self.scene_version,self.node_count))} else {None}),
    {
        if caller!=self.owner || !self.document.is_open() {return None;}
        Some((self.scene_version,self.node_count))
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
