// Failed-capability snapshot; not part of the accepted verification crate.
// This changes only BTreeMap construction (bulk sorted map) while preserving
// the production traversal, oracle, finite input classes, safety checks and
// unwinding assertions.
#[kani::proof]
#[kani::unwind(4)]
fn two_node_ancestry_sorted_from() {
    use super::*;
    let a:u8=kani::any();let b:u8=kani::any();
    let parent=|v:u8|if v%4==0 {None} else {Some((v%4-1) as u64)};
    let pa=parent(a);let pb=parent(b);
    let parents=BTreeMap::from([
        (NodeId(0),pa.map(NodeId)),
        (NodeId(1),pb.map(NodeId)),
    ]);
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
    std::mem::forget(parents);
}
