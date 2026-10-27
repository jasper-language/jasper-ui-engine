use vstd::prelude::*;
use std::collections::BTreeMap;
verus! {
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

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
    requires vstd::laws_cmp::obeys_cmp::<NodeId>(),
    ensures result==ancestor_with_budget(parents@,Some(node),ancestor,parents@.len()),
{
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
