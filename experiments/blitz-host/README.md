# Jasper UI Blitz native facade candidate

The caller consumes only crate-local typed values. NativeTextView privately maps
text, transient preedit display, appearance, host size and DPI to pinned Blitz
native mutator/viewport APIs. No HTML serialization/parser, backend types or CSS
strings escape into the caller. Package names remain provisional.

The native CPU diagnostic uses an explicitly injected Noto CJK fixture and
Blitz paint_scene / AnyRender / Vello CPU. CPU is opt-in through cpu-pending;
no Canvas API or host window/compositor is implemented. Output RGBA bytes have
checked dimensions/budget, but upstream scratch allocation is external TCB.

Text and preedit are supplied display snapshots, not authoritative editable
documents. Clearing preedit clears only a presentation node, not a service cancel
acknowledgment. Candidate windows remain owned by the host input service. No
editor core, IME engine, public service or Signal implementation is included.
The native preedit entry now consumes the existing UiInputBinding admission lease
and compares the trusted session plus Jasper target/generation/document/revision
before rendering; same-node reopens cannot consume old-generation leases. The binding fixture rejects
foreign sessions/stale focus and rotates generations on focus replacement/return.
This remains an unaccepted native bridge, not OS focus or external cancel ACKs.
The existing root engine supplies admission types and Scene/ScenePatch/style/semantic
contracts. Its layout/render algorithms are not called by this Blitz diagnostic.
NativeScene imports existing Container/Text/Rect snapshots and consumes typed
text/style patches with owner/version preflight. The CLI now exercises this path
before the separate external-text client fixture. Ordered Insert/UpdateText/UpdateStyle/Remove frames now map native structure
after complete source-ID topology preflight. The CLI exercises mixed insertion/removal,
semantic activation, exact fresh-import pixel comparison, host viewport scroll
restoration and injected strided software-buffer output. Host viewport scroll
is distinct from unsupported source node SetScroll. Full node input/capture
routing and native OS accessibility remain incomplete. All adapters stay unaccepted candidates.

ViewSlot owns the private BaseDocument and permanently retires it on close.
Native diagnostics reject late text/preedit/style/size/DPI/render operations.
Scoped new/close/is_open/read/write borrow proof evidence exists separately under
evidence/ui-view-*-kernel and chain. These value/ownership proofs do not verify
third-party destruction, host tasks, pixels or the entire adapter.

Default-off candidate only: native execution/preview is not full six-stage
acceptance. Appearance/DPI/preedit mapping still lacks that complete chain.
All heavy compilation, diagnostics and verification use scripts/verify-in-vm.sh:
3G memory, zero swap,128 tasks,1 job,180s default; optional time up to900s
never raises resource caps. No desktop-host compilation or tests.

The host focus-close candidate revokes the exact existing binding before clearing
preedit display. A stale close cannot clear a successor; an already closed view
leaves the host binding untouched. This is synchronous consumer cleanup, not an
OS focus implementation or external service cancellation acknowledgment. Its
whole native pairing method remains unaccepted pending the complete proof chain.

Selection display candidate consumes a host-admitted UTF-8 byte range and maps it
to Blitz's public inline-root selection API. It reads the existing backend display
layout for boundary checks; it does not own another selection/editor document.
The CJK diagnostic visibly highlights the supplied range. Split UTF-8 offsets,
out-of-range positions, stale revisions and closed views are rejected. Snapshot
replacement clears selection. Collapsed caret, grapheme/affinity, cross-node ranges
and the new adapter's full verification chain remain pending.

Display hit candidate delegates a host-admitted logical point to Blitz public
find_text_position and returns only a matching display-root UTF-8 byte offset.
It rejects nonfinite points, stale contexts and closed views. Host coordinate
conversion/routing remains external; no editor caret state is modified. Glyph
geometry, affinity and complex text correctness remain upstream TCB/coverage
limits; this native adapter is not yet fully accepted.

Typed layout candidate maps provisional ContentFlow Row/Column plus integer gap
to the private upstream flex style API. It does not implement flex geometry.
The capped-VM diagnostic compares row/column frames and exact restoration; closed
views reject the update. This two-node facade is not a generic component tree,
and the new adapter's complete ordered verification is still pending.

ComponentUpdate is a provisional typed Framework-to-Rust update value for
appearance and flow. apply_component_update checks trusted caller/expected view
identity before dispatch; raw style/flow setters are private. This consumes
updates and implements no Signal runtime. Native diagnostic passes, but the
whole dispatch proof and generic component tree remain pending.

The borrowed ComponentUpdate label variant maps supplied accessibility metadata
to the private backend attribute; query/remove are checked in the native VM
fixture. This is not a platform accessibility bridge or complete semantic tree.
No OS integration or screen-reader acceptance is claimed; full proofs pending.

Component updates now require an independent expected UI sequence, since matching
external text revision alone cannot order delayed appearance/layout updates.
Successful dispatch advances once; rejection leaves sequence unchanged and source
uses checked_add before mutation. The native fixture does not reach actual MAX
sequence exhaustion. apply_now exists only in the diagnostic host; real producers
must retain originating versions. Complete sequence/dispatch proof remains pending.

Authorized selected-display readback delegates to upstream get_selected_text.
The CJK fixture reads the exact selected text after style/layout updates and
rejects foreign/stale/closed reads. It does not touch a clipboard or implement
an editor/public text service. Complete readback proof remains pending.

Content scroll candidate consumes an admitted logical position and delegates to
Blitz viewport scrolling. Host supplies a valid content extent; this setter does
not claim content-bound clamping. Native long-text frame changes/restores exactly;
invalid/foreign/closed inputs reject. No scroll algorithm/window system added.
Full implementation proof and ordered acceptance remain pending.

Preedit callbacks now retain UiCompositionToken, accepted by the view's persistent
existing UiCompositionSlot. Focus tickets alone cannot distinguish cancelled and
successor preedit within one focus. Explicit UI cancel invalidates the token;
old updates/cancels preserve successor pixels. Snapshot/focus close invalidates
correlation; permanent close retires it. This is UI presentation ownership, not
an IME/input-service implementation. Native pairing proofs remain pending.
The synchronous diagnostic helper refreshes tokens; real delayed callbacks must
keep their originating token rather than using that helper.

The package now also exposes a default-off library candidate. Its `host::SceneHostView`
is for embedding-host consumers; application declarations remain typed Jasper
Scene/ScenePatch values. Backend modules stay private, and exported signatures
contain no Blitz/DOM/CSS/window/GPU types. The external integration test consumes
only these public contracts in CPU and no-CPU modes. Forwarding wrappers still
lack complete implementation-level ordered acceptance; names are provisional.

The active CLI now imports only the typed library for both Scene and external
text/input/capture producer fixtures. It declares no private backend modules.
The former private CLI fixture is archived outside Cargo targets in diagnostics/.
A complete library unit/integration run exposed and fixed a test dependency on
that binary-only helper; production admission was not weakened. This migration
is still prototype evidence, not full native implementation acceptance.
