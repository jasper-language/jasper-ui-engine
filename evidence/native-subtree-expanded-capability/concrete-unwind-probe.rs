// Exploratory fixed-case probe; not an acceptance substitute.
// It used BTreeMap collection rather than the production harness's direct
// insertion, so its program shape is larger and it is not used as a bound proof.
#[kani::proof]
#[kani::unwind(8)]
fn concrete_unwind_probe() {
    use super::*;
    let parents = [
        (NodeId(0), Some(NodeId(1))),
        (NodeId(1), Some(NodeId(0))),
    ].into_iter().collect::<BTreeMap<_, _>>();
    let actual = source_subtree_contains(&parents, NodeId(0), NodeId(3));
    assert!(!actual);
    std::mem::forget(parents);
}
