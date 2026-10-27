# Scoped production UI view ownership verification

actual module imports src/native_adapter/view_slot.rs directly. Verus
extracts its exact new/close/is_open bodies and adds only specifications plus an
opaque ghost projection of the private owned value. No alternate executable
slot/model is substituted. Initial annotation failures are retained separately.
Verus proves logical retirement over generic T, not native Drop effects.

Run order in evidence/ui-view-slot-chain: Verus+Z3, Proptest (256 bounded state
traces), Miri (owned-drop and late access fixture), cargo-fuzz (11s, max128), Kani
(1 harness), then scoped Loom applicability review. State traces also exercise
real as_ref/as_mut, but their exact-reference identity is not included in the
Verus specifications. Thus the whole module/adapter is NOT formally accepted.

Loom is inapplicable to this slot's sequential exclusive-borrow state: no own
concurrency primitives or production interleaving. Third-party T/native rendering
concurrency is excluded, NOT passed. No empty concurrency model is supplied.
All heavy verification runs only via the capped VM entry.

Read-borrow follow-up: evidence/ui-view-read-borrow-chain records the ordered
chain against the current source, with Verus UI-VIEW-READ-001 proving as_ref
value/occupancy correspondence (not physical pointer identity). The fuzz corpus
is retained under evidence. as_mut mathematical specification, native Drop,
host cancellation and the complete adapter remain pending.

Mutable-borrow follow-up: evidence/ui-view-write-borrow-chain records the ordered
chain with Verus UI-VIEW-WRITE-001, relating the actual as_mut return value to
the slot before and after the borrow. This is not physical pointer identity,
third-party destructor verification, host cancellation or complete adapter acceptance.
Initial annotation failure is retained separately.
