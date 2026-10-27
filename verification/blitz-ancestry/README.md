# Production ancestry verification scope

`combined.rs` verifies `source_subtree_contains` from the private Blitz scene adapter.
The executable body is production code; only ghost statements, loop invariants,
a result contract and a decreases measure are added. The specification follows
at most `parents.len()` parent edges and checks the current identity before
consuming an edge. This covers absent entries and cycles without assuming an
acyclic map. It does not prove that scene mutations preserve source topology.

NodeId comparison methods are the verbatim PartialEq/PartialOrd/Ord blocks in
`evidence/blitz-node-id-comparison-pending/expanded.rs`, generated in the capped VM
from the marked declaration in `src/scene.rs` using `rustc -Zunpretty=expanded`.
Compiler/version/output hashes are recorded. Spec trait implementations and the
order-law lemma discharge the BTreeMap key-model requirement; no own-code axiom,
assume, external body or unresolved comparison precondition is used in combined.rs.

Runtime build extraction is in `../native-activation/build.rs`. Proptest and Miri
exercise the actual function, declaration and real maps; fuzz adds dense graphs.
Kani uses a two-entry real BTreeMap with symbolic parents, start and target, and an
independent three-visit scalar oracle. Direct insertion excludes unrelated
iterator collect/sort and oracle-container implementation. Map destruction is
outside the borrowed function's scope and is omitted in this harness. The initial
larger harness timeout remains in evidence. Unwinding assertions stay enabled.

All compiler/solver/test commands run through `scripts/verify-in-vm.sh`; never run
these workloads on the desktop host. Evidence records ordered gate outcomes and
limitations. A bounded Kani result is not an unbounded implementation proof.

`visibility.rs` additionally verifies the exact `NativeScene::source_visible`
body against a recursive relation over its actual parent, displayed and semantics
map views. It consumes at most the parent-map count, requires every visited node
to have both source visibility flags and a parent entry, and succeeds only on a
root `None`. Cycles or incomplete metadata fail closed. This is source metadata
eligibility for focus/actions, not Blitz computed visibility geometry. Production
body equality after ghost erasure and compiler-derived key-law evidence are
recorded separately; no own-code comparison assumption is introduced.
