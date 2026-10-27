use std::collections::{BTreeMap,BTreeSet};
include!(concat!(env!("OUT_DIR"),"/ancestry.rs"));
pub fn check(entries:&[(u64,Option<u64>)],start:u64,target:u64) {
    let parents:BTreeMap<_,_>=entries.iter().map(|(id,parent)|(NodeId(*id),parent.map(NodeId))).collect();
    let actual=source_subtree_contains(&parents,NodeId(start),NodeId(target));
    let mut seen=BTreeSet::new();let mut current=Some(NodeId(start));let mut expected=false;
    while let Some(node)=current {
        if node.0==target {expected=true;break;}
        if !seen.insert(node) {break;}
        current=parents.get(&node).copied().flatten();
    }
    assert_eq!(actual,expected);
}
pub fn fuzz(data:&[u8;20]) {
    let entries:Vec<_>=(0..8).filter(|i|data[*i]&1!=0).map(|i|(i as u64,
        if data[i+8]&1==0 {None} else {Some((data[i+8]%10) as u64)})).collect();
    check(&entries,(data[18]%10) as u64,(data[19]%10) as u64);
}
#[cfg(test)]
mod tests {
    use super::*;use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn ancestry_model(entries in prop::collection::vec((any::<u64>(),prop::option::of(any::<u64>())),0..16),start in any::<u64>(),target in any::<u64>()) {
            check(&entries,start,target);
            for (id,parent) in &entries {check(&entries,*id,target);if let Some(parent)=parent {check(&entries,*id,*parent);}}
        }
        #[test]
        fn dense_ancestry_model(data in any::<[u8;20]>()) {fuzz(&data);}
    }
    #[test]
    fn miri_ancestry() {
        check(&[],u64::MAX,u64::MAX);check(&[],0,1);
        check(&[(0,Some(1)),(1,Some(0))],0,2);
        check(&[(u64::MAX,Some(0)),(0,None)],u64::MAX,0);
        for x in 0..32 {fuzz(&[x;20]);}
    }
}
#[cfg(kani)]
mod proofs {
    #[kani::proof]
    #[kani::unwind(4)]
    fn two_node_ancestry() {
        use super::*;
        let a:u8=kani::any();let b:u8=kani::any();
        let parent=|v:u8|if v%4==0 {None} else {Some((v%4-1) as u64)};
        let pa=parent(a);let pb=parent(b);
        // Real production map, built directly: exclude unrelated collect/sort/oracle containers.
        let mut parents=BTreeMap::new();parents.insert(NodeId(0),pa.map(NodeId));parents.insert(NodeId(1),pb.map(NodeId));
        let start=(kani::any::<u8>()%4) as u64;let target=(kani::any::<u8>()%4) as u64;
        let actual=source_subtree_contains(&parents,NodeId(start),NodeId(target));
        let mut current=Some(start);let mut expected=false;
        for _ in 0..3 {
            if let Some(node)=current {
                if node==target {expected=true;break;}
                current=if node==0 {pa} else if node==1 {pb} else {None};
            }
        }
        assert_eq!(actual,expected);
        // The production function only borrows the map; destruction is outside this scope.
        std::mem::forget(parents);
    }
}

