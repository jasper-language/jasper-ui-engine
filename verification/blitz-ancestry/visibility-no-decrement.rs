use vstd::prelude::*;
use vstd::std_specs::cmp::*;
use vstd::laws_cmp::*;
use core::cmp::Ordering;
verus! {
impl<T> ViewSlot<T> { pub closed spec fn owned_value(&self) -> Option<T> { self.value } }



use std::collections::BTreeMap; pub mod blitz_dom {#[verifier::external_body] pub struct NodeId {}} #[verifier::external_body] pub struct BaseDocument {}
pub closed spec fn native_scene_metadata_same(a:&NativeScene,b:&NativeScene)->bool {a.owner==b.owner && a.scene_version==b.scene_version && a.node_count==b.node_count && a.parents@==b.parents@ && a.children@==b.children@ && a.displayed@==b.displayed@ && a.semantics@==b.semantics@ && a.nodes@==b.nodes@}
pub(crate) mod native_scene {pub(crate) use super::NativeScene;}
pub closed spec fn scene_host_close_post(a:&SceneHostView,b:&SceneHostView,owner:u64,result:bool)->bool {result==(owner==a.inner.owner && a.inner.document.owned_value().is_some()) && (if owner==a.inner.owner {b.inner.document.owned_value().is_none()} else {b.inner.document.owned_value()==a.inner.document.owned_value()}) && native_scene_metadata_same(&a.inner,&b.inner)}
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

#[derive(Clone,Copy)]
pub struct NodeId(pub u64);
impl PartialEqSpecImpl for NodeId {
    open spec fn obeys_eq_spec()->bool {true}
    open spec fn eq_spec(&self,other:&Self)->bool {self.0==other.0}
}
impl PartialOrdSpecImpl for NodeId {
    open spec fn obeys_partial_cmp_spec()->bool {true}
    open spec fn partial_cmp_spec(&self,other:&Self)->Option<Ordering> {Some(OrdSpec::cmp_spec(&self.0,&other.0))}
}
impl OrdSpecImpl for NodeId {
    open spec fn obeys_cmp_spec()->bool {true}
    open spec fn cmp_spec(&self,other:&Self)->Ordering {OrdSpec::cmp_spec(&self.0,&other.0)}
}
impl ::core::cmp::PartialEq for NodeId {
    #[inline]
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
impl Eq for NodeId {}
impl ::core::cmp::PartialOrd for NodeId {
    #[inline]
    fn partial_cmp(&self, other: &Self)
        -> ::core::option::Option<::core::cmp::Ordering> {
        ::core::option::Option::Some(::core::cmp::Ord::cmp(self, other))
    }
}
impl ::core::cmp::Ord for NodeId {
    #[inline]
    fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
        ::core::cmp::Ord::cmp(&self.0, &other.0)
    }
}
proof fn node_id_order_model()
    ensures obeys_cmp::<NodeId>(),
{
    reveal(vstd::laws_eq::obeys_eq_spec_properties);
    reveal(obeys_partial_cmp_spec_properties);
    reveal(obeys_cmp_partial_ord);
    reveal(obeys_cmp_ord);
}

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
    document:ViewSlot<BaseDocument>,
    owner:u64,
    scene_version:u64,
    node_count:usize,
    parents:BTreeMap<NodeId,Option<NodeId>>,
    children:BTreeMap<NodeId,Vec<NodeId>>,
    displayed:BTreeMap<NodeId,bool>,
    semantics:BTreeMap<NodeId,Semantics>,
    nodes:BTreeMap<NodeId,(blitz_dom::NodeId,Option<blitz_dom::NodeId>)>,
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

impl SceneHostView {
    pub fn close(&mut self,owner:u64)->(result:bool)
 ensures scene_host_close_post(old(self),final(self),owner,result),
 {proof {reveal(scene_host_close_post);}self.inner.close(owner)}
}


spec fn visible_walk(parents:Map<NodeId,Option<NodeId>>,displayed:Map<NodeId,bool>,semantics:Map<NodeId,Semantics>,current:Option<NodeId>,remaining:nat)->bool
    decreases remaining,
{
    match current {
        None=>true,
        Some(cursor)=>remaining>0 && displayed.dom().contains(cursor) && displayed[cursor]
            && semantics.dom().contains(cursor) && !semantics[cursor].hidden && parents.dom().contains(cursor)
            && visible_walk(parents,displayed,semantics,parents[cursor],(remaining-1) as nat),
    }
}

impl NativeScene {
fn source_visible(&self,id:NodeId)->(result:bool)
    ensures result==visible_walk(self.parents@,self.displayed@,self.semantics@,Some(id),self.parents@.len()),
    { proof { node_id_order_model(); }
        let mut current=Some(id);let mut remaining=self.parents.len();
        loop
            invariant
                obeys_cmp::<NodeId>(), remaining<=self.parents@.len(),
                visible_walk(self.parents@,self.displayed@,self.semantics@,current,remaining as nat)
                    ==visible_walk(self.parents@,self.displayed@,self.semantics@,Some(id),self.parents@.len()),
            decreases remaining,
        {
            proof { reveal_with_fuel(visible_walk,2); }
            let cursor=match current {Some(cursor)=>cursor,None=>return true};
            if remaining==0 {return false;}

            if !match self.displayed.get(&cursor) {Some(value)=>*value,None=>false} {return false;}
            if match self.semantics.get(&cursor) {Some(value)=>value.hidden,None=>true} {return false;}
            current=match self.parents.get(&cursor) {Some(parent)=>*parent,None=>return false};
        }
    }
}
}
