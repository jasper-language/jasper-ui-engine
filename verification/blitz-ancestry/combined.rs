use std::collections::BTreeMap;
use vstd::prelude::*;
use vstd::std_specs::cmp::*;
use vstd::laws_cmp::*;
use core::cmp::Ordering;
verus! {
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
spec fn ancestor_with_budget(parents:Map<NodeId,Option<NodeId>>,current:Option<NodeId>,ancestor:NodeId,remaining:nat)->bool
    decreases remaining,
{
    match current {
        None=>false,
        Some(entry)=>entry==ancestor || (remaining>0 && parents.dom().contains(entry)
            && ancestor_with_budget(parents,parents[entry],ancestor,(remaining-1) as nat)),
    }
}
fn source_subtree_contains(parents:&BTreeMap<NodeId,Option<NodeId>>,node:NodeId,ancestor:NodeId)->(result:bool)
    ensures result==ancestor_with_budget(parents@,Some(node),ancestor,parents@.len()),
{
    proof { node_id_order_model(); }
    let mut current=Some(node);let mut remaining=parents.len();
    loop
        invariant
            vstd::laws_cmp::obeys_cmp::<NodeId>(),
            remaining<=parents@.len(),
            ancestor_with_budget(parents@,current,ancestor,remaining as nat)
                ==ancestor_with_budget(parents@,Some(node),ancestor,parents@.len()),
        decreases remaining,
    {
        proof { reveal_with_fuel(ancestor_with_budget, 2); }
        let cursor=match current {Some(cursor)=>cursor,None=>return false};
        if cursor.0==ancestor.0 {return true;}
        if remaining==0 {return false;}
        remaining-=1;
        current=match parents.get(&cursor) {Some(parent)=>*parent,None=>None};
    }
}
}
