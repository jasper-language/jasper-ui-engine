# Native Blitz adapter acceptance inventory

obligations.json maps every current native_view.rs method to an explicit obligation
and exact source hash. The read-only component_version getter has a scoped implementation proof; the
logical normal-return close also has scoped chain evidence. The other 44 remain
incomplete for six-stage acceptance. Scoped scalar/slot/admission evidence must not be promoted
to whole-adapter proof. Native diagnostics are integration evidence only.

The third-party boundary covers upstream implementation internals, not Jasper's
own method ordering, rejection frame or lifecycle composition. In particular,
mutator/resolve panic rollback is not specified or proved, while the current DPI interval is a candidate policy rather than a proved float
contract. These remain open correctness/specification work.

No VM/compiler/model run is claimed by this inventory. Existing evidence stays
in evidence/. No public namespace or external service protocol is frozen here.

Float capability audit: the capped-VM probes float-capability.rs and
float-comparison-capability.rs are NOT production proofs. Current Verus rejects
is_finite without a std spec; the direct-comparison probe also fails its
postcondition. Evidence is in evidence/blitz-float-proof-capability. No own-code
assume_specification/external_body workaround or gate pass is claimed.

Reusable proof correspondence check: `python3 scripts/check-blitz-proof-mapping.py`.
It rejects changed extracted production bodies, dependency bodies, contracts/proof text,
missing reports and reports without a proved result. It runs no solver and does not
replace any of the six acceptance stages or accept the native adapter. The mappings include the public version/close projections and scene close dependency, plus the standalone hidden-ancestry helper.
Public-body drift, dependency drift, missing evidence and nonproved-report negative
controls are reproducible with `python3 scripts/test-blitz-proof-mapping.py` and
recorded in `evidence/blitz-proof-mapping-public-audit/report.json`. Changes outside an extracted body
still require their own review and verification; a matching scoped proof is not a
whole-file or whole-adapter acceptance claim.

The first direct facade-body proof is `native-component-version` in
scripts/verify-version.py. It extracts the actual NativeTextView field declaration
and component_version body without changing executable statements. The contract
states that a live slot returns the stored version and a retired slot returns None.
BaseDocument and Blitz NodeId are opaque third-party payload types in this proof:
no backend methods, constructors, destructors, representation or layout properties
are assumed. Own ViewSlot and composition/input declarations use their real source.
This is a read-only field projection argument, not a proof of the constructor,
updates, native close, or physical rendering. Evidence:
`evidence/native-component-version-kernel/report.json` (26 verified including
dependencies, two negative controls rejected). The seven real native regression
tests passed after moving the unchanged getter into a separately marked impl;
those tests do not replace the remaining ordered gates.

`pointer_from_host` now dispatches typed mouse phases to pinned Blitz EventDriver.
Eight native regressions pass, including owner/stale/closed rejection and cancel/up
active-state cleanup. It remains an unaccepted candidate; capture, application event
callbacks, touch/pen/modifiers and true screen coordinates remain pending. No OS
window capture or service implementation is claimed.

PointerHostBridge now consumes a synchronous authenticated host-capture interface.
It requests capture before delivery, cancels/releases on focus loss, rejects stale
focus/capture tickets, and retains failed release state for retry (including failed
Down with no returned ticket). Tests combine a recording host with real Blitz;
no actual OS capture backend is claimed. Eight bridge methods remain unproved and
unaccepted; host failures, partial focus-loss outcome and integration obligations
are recorded in evidence/blitz-host-capture-pending/report.json.

Typed Framework pointer callbacks now project real Blitz events into own context,
phase and view-part values, with prevent-default/stop-propagation replies. Ten
native tests pass, including callback admission and default-selection suppression.
The capture bridge can now receive a separate typed Framework sink; general
component routing and callback panic semantics remain pending.
See evidence/blitz-framework-events-pending/report.json; no six-stage pass claimed.

Capture/Framework callback paths are now connected through PointerHostBridge::with_sink.
The recording capture host and application event consumer remain separate objects.
The real Blitz Down/Move/Cancel/Down/Up trace is checked across focus loss, denied
release, repeated cancellation, stale callbacks and successful retry. This remains
a default-off unaccepted candidate; evidence/blitz-capture-framework-path-pending/.

Admitted external focus now projects into Blitz, and focused command keys project
back into typed Framework down/up callbacks. Focus closure clears native focus;
late keys are rejected. Twelve native regressions pass. Modifiers/key location,
focus-change callbacks, OS focus requests and full own-code verification remain
pending. Text commits stay with external services. Evidence:
`evidence/blitz-focused-keys-pending/report.json`.

Common Shift/Control/Alt/Meta flags and Standard/Left/Right/Numpad locations now
round-trip through actual Blitz key callbacks. All 64 projection combinations and
the 13-test native suite pass in the capped VM. Extended lock/AltGraph modifiers,
real keyboard platform tests and full own-code acceptance remain pending. The
explicit keyboard-types 0.8.3 dependency names the version already in the Blitz
lockfile (MIT OR Apache-2.0), stays private and is unproved upstream TCB.
Evidence: evidence/blitz-key-modifiers-pending/report.json.

Admitted native focus transitions now notify Framework consumers with own ticket
and focused flag. The pointer bridge connects focus loss to cancellation, native
focus/preedit cleanup and notification; denied release is retained for retry
without duplicate focus-loss callbacks. Fourteen native regressions pass. This
is native view focus, not an OS window focus request or service acknowledgement;
full own-code acceptance remains pending. Evidence:
evidence/blitz-focus-notifications-pending/report.json.

Explicit owner-gated host retirement now blocks successor capture before view
teardown. Failed release state survives destruction for retry; successful cleanup
permanently retires the bridge. Cross-application retirement rejection and other
application pixels are checked. This is an explicit client lifecycle entry, not
Drop cleanup or an OS service. External text focus still needs caller closure.
Evidence: evidence/blitz-host-retirement-pending/report.json; full gates pending.

Capture admission ordering regression fixed: a same-session stale view context
previously reached the recording host before downstream view rejection. Native
owner/context checks now precede binding allocation and capture requests. Four
mismatch dimensions produce zero capture/release requests; the full 16-test suite
passes. Evidence: evidence/blitz-capture-admission-order-pending/report.json.
This fix remains a candidate until its complete own-code verification gates pass.

Framework pointer events now include view-local logical position and primary-button
state. The 2x-scale/nonzero-scroll callback test and 17-test native suite pass.
Outside-content capture delivery/original target retention and final release sample
coordinates remain explicit next checks; complete own-code gates are pending.
Evidence: evidence/blitz-pointer-coordinates-pending/report.json.

Captured Framework Move/Up/Cancel now retain the initial Jasper view part outside
content bounds. The reproduced dropped-callback failure is fixed, and up_at carries
the final release position; nonfinite release samples do not clear capture. Eighteen
native regressions pass. This does not retarget or prove Blitz default actions;
no-hit Down, actual OS capture loss and complete own-code gates remain pending.
Evidence: evidence/blitz-captured-target-pending/report.json.

No-hit Framework Down now cancels native press state and compensates host capture
instead of returning a ticket that can acquire a later component. Failed release
stays retryable without permitting moves; the no-target/recovery regression and
19-test suite pass. Full own-code acceptance remains pending. Evidence:
evidence/blitz-no-target-capture-pending/report.json.

The next component slice now declares an actual native Blitz button through typed
Rust input. Its versioned caption update changes pixels and cannot write text-view
document snapshots. Twenty native tests pass. Button activation callbacks, disabled
semantics, generic child trees and complete own-code acceptance remain pending.
Evidence: evidence/blitz-typed-button-pending/report.json.

ButtonDisabled now maps the native disabled attribute through versioned component
updates; focus and command-key entry reject disabled controls. Native focus clears
without revoking the external service lease. Activation/disabled pointer policy,
consumer focus notifications and full acceptance remain pending. Evidence:
evidence/blitz-button-disabled-pending/report.json.

Actual Blitz button Click events now reach a typed activation callback; disabled
and cancelled presses do not activate. Twenty-two native regressions pass after
sharing pointer dispatch. Keyboard/assistive activation, capture-bridge activation
wiring, generic trees and full own-code gates remain pending. Evidence:
evidence/blitz-button-activation-pending/report.json.

A provisional Framework Enter binding now activates admitted focused native buttons,
rejecting IME composition, repeat, shortcut modifiers and disabled/stale focus.
Twenty-three native tests pass. Space/assistive activation, capture activation
wiring and full own-code gates remain pending; this is not a frozen public policy.
Evidence: evidence/blitz-button-enter-pending/report.json.

Own owner-gated view semantics now expose text/button role, accessible name,
disabled/native-focus state, activation capability and component update version.
Explicit empty names remain empty. Twenty-four native regressions pass. This is
neither a platform accessibility bridge nor a complete semantic tree; own-code
acceptance remains pending. Evidence: evidence/blitz-own-semantics-pending/report.json.

The current-source read-only component_version getter now has an ordered rerun
of Verus+Z3, Proptest, Miri, fuzz and Kani, followed by actual-method Loom N/A
rationale. Exact own field/body mapping and opaque unused third-party payload
limitations remain explicit. This backfill does not increase functional completion.
Evidence: evidence/native-component-version-chain/report.json.

TextDisplayView now forwards the existing owner/context-admitted semantics snapshot
through public Jasper-owned DTOs. External CPU/no-CPU diagnostics cover label/version,
native focus, successor context and retirement. This new read wrapper is pending
ordered acceptance; it is not an OS accessibility bridge.
Evidence: evidence/blitz-public-text-semantics-pending/report.json.

SceneHostView command keys now use native EventDriver against admitted current source
focus, with own NodeId/version and modifier/location projection. Final native focus
transitions use the existing notification path. External press/release and rejection
diagnostics pass under CPU/no-CPU configurations. The new scene method, event handler
and public forwarding method remain pending all ordered gates; text commits and OS
input/service ownership are outside this path.
Evidence: evidence/blitz-public-scene-key-pending/report.json.

The standalone hidden-ancestry marker/proof digest now also fails closed on source
or specification drift, including an executable ancestor-OR negative control.
Evidence: evidence/blitz-hidden-proof-mapping-audit/report.json. This is correspondence
checking only; its Kani timeout and incomplete ordered acceptance remain unchanged.

Public backend-type exposure can now be checked against capped-VM rustdoc JSON with
`python3 scripts/check-blitz-public-types.py <rustdoc-json>`. CPU and no-CPU metadata
pass, and an injected backend return type is rejected. Explicit API/trait/enum
signatures are checked; rustdoc-added upstream generic blanket impls are excluded.
This is not a proof stage, a string-semantics audit, or whole-adapter acceptance.
Evidence: evidence/blitz-public-type-boundary/report.json.

The type-boundary checker follows private transparent aliases used in public
signatures as well, with a separate alias-to-backend negative control. This closes
a checker blind spot without changing production Rust or expanding its acceptance.
CPU/no-CPU metadata reuse is recorded in the same boundary report.

The shared cpu_frame_bytes production helper now has exact scalar Verus+Z3/negative
controls followed by Proptest, Miri, fuzz and symbolic Kani, with method-specific
Loom N/A. Text and scene use the same size/budget guard. This accepts only numeric
admission, not the renderer, backend scratch allocations or full wrapper methods.
Evidence: evidence/cpu-frame-bytes-chain/report.json.

The actual cancellation route policy now has scoped ordered acceptance with the
real root identity predicates: rejection precedes delivery/open flags; current and
successor callback routing are distinguished. The native cancel helper consumes
this policy, but backend effects and host-release coordination remain unaccepted.
Evidence: evidence/capture-cancel-policy-chain/report.json.

NativeTextView::host_context_matches now has its own ordered exact-body acceptance
chain, including all full-width identity dimensions and method-specific Loom N/A.
This is read-only host-context equality, not authentication, backend correctness
or acceptance of its lifecycle/capture callers.
Evidence: evidence/native-host-context-chain/report.json.

Per-node source Node.scroll/SetScroll now adapt native scroll_offset directly for
snapshots, inserted subtrees and ordered patches. CPU pixels and hit targeting
match fresh import; invalid frames preserve the previous scroll state. Source
offsets currently admit finite nonnegative values through u32::MAX. This candidate
does not implement scroll geometry/clamping/physics; full own-code gates pending.
Evidence: evidence/blitz-source-scroll-pending/report.json.

Typed ScenePatch::UpdateSemantics now updates native attributes and focus participation
for stable roles, including clearing previous labels and disabled state. Structural
role changes remain remove/reinsert; whole-frame rejection is diagnosed, while
full own-code ordered acceptance remains pending.
Evidence: evidence/blitz-source-semantics-pending/report.json.

Follow-up: non-Button semantic roles can change through attributes on the stable
native div tag; prior role attributes are cleared. Only transitions to/from Button
require structural replacement. Ordered implementation acceptance remains pending.
Evidence: evidence/blitz-semantic-role-update-pending/report.json.

Current scoped additions: NativeScene::snapshot_identity and its host facade
wrapper have ordered stored-metadata projection acceptance. The semantic role
tag-preservation guard is separately accepted. These do not prove native tree
correspondence, full semantic mutation, focus queries or keyboard dispatch.
Evidence: evidence/scene-snapshot-identity-chain/report.json,
evidence/scene-host-identity-chain/report.json, evidence/semantic-role-tag-chain/report.json.

Proof correspondence also requires the exact configured negative-control set, successful rejection flags and exit code 1; missing controls and timeout code 124 reject. Optimized-Python controls are in `evidence/blitz-proof-negative-control-admission/report.json`. This is an evidence guard, not a new solver run or UI feature.

Saved kernel source and Verus positive/negative logs are required by the current mapping check. Exact negative source mutation and real postcondition-failure summaries are checked; missing/corrupt artifacts reject. See `evidence/blitz-proof-artifact-admission/report.json`; this does not rerun the solver.

Standalone hidden-ancestry stage1 also requires saved logs/tool observations and hash-matched negative sources with the expected invariant/decreases failures. See `evidence/blitz-ancestry-artifact-admission/report.json`; its remaining acceptance stages stay incomplete.

Checklist item 1 now has a borrowed `SceneUpdate` payload without host owner or Signal runtime. `SceneHostView::apply_update` and the active prototype consume it through the existing scene path. See `evidence/blitz-framework-update-contract-pending/report.json`; complete delegation/mutation acceptance remains pending.

The diagnostic crate is now consumer-only: its library re-exports the root `native` facade, the active scene prototype uses `engine::BlitzEngine`, and its CPU feature forwards to the root engine. Native unit tests belong to the root library; the diagnostic crate retains external caller tests. No whole-engine acceptance is implied.

`native-backend-frame-import` proves the actual private Headless frame-state replacement method, with exact extraction and ordered runtime validation in `evidence/native-backend-frame-chain/report.json`. Its bounded Kani domain and method-specific Loom nonapplicability are explicit. Trait forwarding, native rendering/presentation and the whole engine remain unaccepted.
