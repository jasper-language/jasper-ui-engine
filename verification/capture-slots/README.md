# Capture storage candidate verification

This crate includes the actual unexported candidate runtime source and exact
production Capture/NodeId declarations extracted by `generate-types.py`.
It does not verify or replace the current BTreeMap-based production router.

Run gates in order: the Verus probe, native property tests, targeted Miri,
cargo-fuzz, Kani, then review Loom applicability. The two-key functional target
keeps arbitrary u64 keys/query/optional targets and normal Vec/Capture drop;
no `mem::forget`, surrogate map or single-key restriction. Property operation
sequences also compare targets and nonempty paths against std BTreeMap.

This candidate has no interior mutability, atomics, shared handles, threads or
host callbacks. All mutation requires exclusive `&mut self`; therefore no Loom
concurrency path exists in this candidate. This rationale does not cover future
shared production router/host integration, which must be assessed separately.

Finite operation tests/fuzz and bounded Kani are complementary checks, not a
replacement for the Verus implementation contract. All stages and production
migration remain pending until their source-matched evidence is recorded.
