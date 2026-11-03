// Diagnostic harness snapshot; it was appended to `mod proofs` of
// verification/native-activation/src/ancestry.rs for these runs only and is not part of the chain.
// Real map, real insertion, real drop (no mem::forget), original symbolic domain, independent oracle.
fn real_map_case(a:u8,b:u8,start:u64,target:u64) {
    use super::*;
    let parent=|v:u8|if v%4==0 {None} else {Some((v%4-1) as u64)};
    let pa=parent(a);let pb=parent(b);
    let mut parents=BTreeMap::new();parents.insert(NodeId(0),pa.map(NodeId));parents.insert(NodeId(1),pb.map(NodeId));
    let actual=source_subtree_contains(&parents,NodeId(start),NodeId(target));
    let mut current=Some(start);let mut expected=false;
    for _ in 0..3 {
        if let Some(node)=current {
            if node==target {expected=true;break;}
            current=if node==0 {pa} else if node==1 {pb} else {None};
        }
    }
    assert_eq!(actual,expected);
}
#[kani::proof]
fn real_map_full_domain() {
    let (a,b,s,t):(u8,u8,u8,u8)=(kani::any(),kani::any(),kani::any(),kani::any());
    real_map_case(a,b,(s%4) as u64,(t%4) as u64);
}
