# Host Surface Lifecycle Verification

`src/lib.rs` includes the production `src/surface_lifecycle.rs` copied by `build.rs`; only leading
module documentation comments are converted to ordinary comments so the body can be embedded in a
submodule. No behavior is duplicated.

Checks:

- Proptest in the production module: successful resize/suspend/resume cycles remain active and
  strictly advance generations.
- Kani on the same extracted production source: owner/stale rejection preserves state, successful
  transitions advance, and retirement invalidates rendering.
- Miri can execute the same pure, allocation-free state machine.
- `fuzz/fuzz_targets/host_surface.rs` drives arbitrary owner bytes and lifecycle command streams
  through the production state machine, checking rejection preservation and render eligibility.

Loom is not applicable because the state machine has no shared or concurrent state. The broader
`HostRenderer` orchestration around Blitz/wgpu/Surface is not covered by these finite checks.
