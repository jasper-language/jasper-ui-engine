use vstd::prelude::*;
use std::collections::BTreeMap;
verus! {
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

fn source_subtree_contains(parents:&BTreeMap<NodeId,Option<NodeId>>,node:NodeId,ancestor:NodeId)->bool {
    let mut current=Some(node);let mut remaining=parents.len();
    while let Some(node)=current
        decreases remaining,
    {
        if node==ancestor {return true;}
        if remaining==0 {return false;}

        current=match parents.get(&node) {Some(parent)=>*parent,None=>None};
    }
    false
}
}
