# UI input identity admission slice

Extracts the exact current `accepts_ui_input` body, NodeId and ResourceGate into
one verification crate. No shadow gate or alternate production algorithm.

The current UI context is host-trusted. The predicate checks authenticated caller
plus resource session/slot/generation and target ID/incarnation, document ID and
revision. Host focus closure must revoke/retire the gate; host document changes
must update the current context before accepting later results. The predicate
alone does not perform authentication, dispatch, edit transactions or cancellation.
A gate must never be duplicated/recreated while an old ticket could still exist.

Proptest, Miri, fuzz and Kani exercise the real gate close/reopen/retire flow as
well as every independent stale identity dimension. Exact all-input predicate
and gate transition contracts are verified with Verus+Z3 via verify-version.py.
No test/bounded checking is described as replacing those contracts.

Loom applicability (last stage): this slice has no threads, atomics, interior
mutability, shared callback queues or concurrency implementation. Admission has
only shared immutable inputs, and the actual gate state changes require exclusive
`&mut`. There is no actual production synchronization path to instrument with Loom;
therefore mark not-applicable with this source-matched rationale rather than add
an invented mutex/model. Future dispatch/async host integration must supply a
separate real-path Loom review. This does not claim concurrent lifecycle safety.

Run heavy commands only through scripts/verify-in-vm.sh (VM tree limit3GiB,
swap0, jobs1 and deadline). Capture-store Kani diagnostics are separately deferred
by the user and must not be relaunched as part of this isolated slice.

## Persistent UI binding

`UiInputBinding` now owns its ResourceGate and current optional context. Bind is
allowed only while vacant, and failed bind preserves prior state. Close accepts
only the exact active ticket/context; replayed tickets cannot close a successor.
Retire permanently disables the slot. This synchronous object publishes state
under exclusive `&mut`; it has no callbacks/threads/atomics/queues, so the same
Loom non-applicability rationale applies to its real implementation. It does not
cancel external tasks, allocate global identities, or authorize recreating a gate
for the same session/slot while previous tickets can exist. Those remain explicit
host obligations. The extended harness checks rejected bind/close framing,
close/reopen with changed revision, stale successor close and retirement.

## Borrowed input payload bridge

`admit` returns a sealed `AdmittedUiInput<'a,T>` borrowing both the actual binding
and original payload. Parametric Verus contracts cover arbitrary T and preserve
payload content plus admitted target; the test harness also checks pointer identity.
Rust lifetime/borrow checking remains an explicit compiler trust boundary.
`borrow-conflict.rs` is a negative compilation test: retaining the admitted payload
across binding.retire must fail specifically with E0502, not merely any error.

The root integration test uses actual normalized Pointer/Key/Text/Composition/
Scroll/Focus event variants. This demonstrates the bridge accepts those payload
types; it does not automatically wire old routers or validate event contents.
Trusted host lifecycle operations (including focus teardown) remain control-plane
operations and are not forced through a closed client input gate. Detached/cloned
async results require a fresh admission check before later consumption.

## Actual KeyboardRouter consumer

The extraction now includes actual FocusManager/KeyboardRouter storage, focus
read/cleanup methods and the admitted input consumer/retirement methods. Native
root tests construct a real Scene/SemanticsTree and router, cover no focus,
matching focus, changed focus, and binding retirement. Kani uses actual structs
with empty traversal-order Vec and arbitrary focused ID; traversal-order framing
for arbitrary sequences is the separate Verus implementation contract.

The new route filters focused target identity and preserves the borrowed payload;
legacy raw route methods retain their trusted low-level API behavior and are not
automatically gated. Host focus changes/incarnation reuse still must rotate/close
the matching binding. `retire_input_binding` is host shutdown, permanently disables
that binding and clears focus; it does not deliver IME/pointer cancellation.
`focus-borrow-conflict.rs` must fail with E0502 if focus changes while returned
payload is still used. No thread/queue/callback/synchronization was added; Loom
remains not applicable to this actual synchronous slice, not to future host code.

## Binding generation rotation

`replace` rotates one active binding under exclusive access. Wrong owner/stale
context or generation exhaustion leaves the original binding intact; MAX is
checked before closing so the last valid lease is not accidentally destroyed.
The new generation invalidates old callbacks even when the node ID is unchanged.
The verification fixture seeds the actual private ResourceGate/UiInputBinding
state with arbitrary generation (including zero/MAX), avoiding 2^64 operations;
it does not introduce a substitute implementation. All relevant runtime branches
still call the real accepts/close/bind methods. This method does not move keyboard
focus or synchronize cross-object host publication. Loom remains not applicable
to this exclusively borrowed synchronous implementation; future host paths differ.

## Coordinated focus transfer

Actual `transfer_input_focus` validates current focused target, read-only focus
eligibility, and the active binding before committing a fresh generation and the
next focus. Every rejection preserves both objects, including MAX generation.
The eligibility and existing focus-selection implementation contracts prove that
the final focus call cannot fail after this preflight on an unchanged exclusive
order; no business assume is added to justify its ignored Result.

Kani uses the actual two-entry Vec<NodeId> order, arbitrary values/duplicates,
arbitrary generation and optional focused ID (unwind3, assertions retained).
Verus proves arbitrary sequence length and full state framing. Root tests use a
real Scene/SemanticsTree and cover invalid target, wrong owner, successful document
transfer, stale callback and routed new payload. Exclusive synchronous borrows
provide publication isolation here; no real concurrent implementation was added,
so Loom remains not applicable. Host initial binding, scene incarnation updates,
trusted raw focus APIs and asynchronous cancellation remain separate obligations.

## Initial publication and normal close

Actual KeyboardRouter begin_input_focus publishes a vacant input binding only
when currently unfocused and target is eligible. close_input_focus validates the
matching active focus/binding, invalidates it, clears focus and returns the full
old UiInputTicket as a cancellation intent (session/slot/generation, target
incarnation, document/revision). Replayed close cannot affect a successor.
The same persistent binding may reopen unless permanently retired/exhausted.
This is an owner-qualified intent, not delivery/acknowledgement or cancellation
of a native engine task. Host control-plane handling must address the old owner
rather than resolving just the node ID against a new scene. No concurrency path
was introduced; exclusive synchronous publication is the actual implementation,
so Loom remains not applicable here and host async processing remains separate.

## UI composition-view correlation

Actual UiCompositionSlot tracks metadata for one preedit/candidate view, not
engine text storage, painting or a generic IME service. Each begin requires an
admitted focus owner and allocates a nonwrapping serial. Even two compositions
under the same focus ticket differ by serial, so a late cancel/update cannot
match its successor. Retain one slot across cancellations; never recreate it
under a live owner while callbacks can remain. Global slot uniqueness is a host
trust boundary, as with persistent ResourceGate lifetime.

accepts checks both current binding and full stored owner/serial. cancel matches
the exact stored token plus authenticated caller and deliberately does not require
the focus binding to remain live: old-view cleanup must still work after focus
closure. This clears UI metadata only, not native tasks or pixels/text owned by
another component. retire returns the complete old cancellation correlation.
Native integration tests cover active retirement; bounded scalar harness currently
reaches retirement after cleanup, while Verus proves the arbitrary active-state
retirement postcondition. No new concurrency path; Loom remains not applicable.

Composition range extension: extracts the unchanged legacy CompositionEvent type and
actual UI COMPOSITION RANGE kernel. Property oracle is Rust str::get(start..end),
with arbitrary Unicode strings (up to 31 scalars) and arbitrary usize offsets;
Miri additionally visits every offset pair in five short ASCII/CJK/emoji/combining
fixtures. Kani uses those five fixed documents, full-width usize offsets and actual
binding/slot lifecycle; this finite document set does not prove arbitrary Unicode.
The existing 65-byte fuzz target selects those documents and byte-sized offsets;
it also continues all older identity drivers. Verus covers arbitrary valid str and
indices, trusting pinned vstd/stdlib UTF-8 boundary and byte slice specifications.
Only replacement byte boundaries are checked, not grapheme boundaries, selection
inside preedit, editor mutation, or the host's document/revision association.
Loom applicability: composition_identity range admission reads immutable str/event
and exclusive local slot/binding state. No thread, atomics, locks, queue or callback
exists in this module path; Loom is N/A. External concurrency requires separate
production integration and verification, not an invented stand-in model.

Preedit-local extension extracts PREEDIT EVENT TYPE and UI PREEDIT ADMISSION KERNEL.
The cursor is Option<(usize,usize)> within event.text, independent of document
replacement coordinates. None must stay None. Unicode property cases use safe
str::get as the local-range oracle; Miri includes CJK/emoji/combining and absent
cursor. Kani uses actual owned PreeditEvent plus three fixed strings (empty, ni,
你), arbitrary full usize cursor bounds and arbitrary presence. Prior document
range harness retains its five-string scope. Fuzz selects five fixed strings and
byte-sized ranges or absence, with the existing seeded identity corpus.
Loom is N/A for this module: immutable owned text borrows, local exclusive state,
no concurrency primitive or callback. Desktop Ime production conversion remains
outside this slice and must be migrated and verified before integration is complete.
New PreeditEvent has no derived Clone implementation; this path only borrows the
payload. Legacy non-Copy Clone remains outside proof scope. The vstd String::as_str
specification is trusted.

Desktop IME mapping: build extracts the actual desktop normalize_ime body, actual
normalized InputEvent and its real production dependency types. winit-ime-type.rs
is the verbatim winit 0.30.13 Ime enum declaration (Apache-2.0), not an alternate
mapper. Formal runner refuses to proceed unless this declaration exactly matches
the pinned Cargo registry source and records both hashes. Actual desktop-feature
integration tests compile against winit itself, not this declaration snapshot.
The new normalized Preedit variant keeps text/local optional cursor separate from
legacy document Composition. The mapper copies even invalid local offsets; scalar
boundary validation remains the downstream admission responsibility, not silent
normalization/clamping. None and empty text preserve hide-cursor/clear intent.
Properties use arbitrary Unicode and cursor bounds; Kani uses three literal texts
and arbitrary usize/presence, and all four Ime variants. Fuzz reuses five literal
texts/byte-sized bounds/optional cursor. These finite sets do not replace the
arbitrary-string Verus contract. The actual String clone specification in vstd is
trusted; unrelated legacy enum/struct derived Clone bodies are not claimed proved.
Loom N/A: normalize_ime is a pure synchronous conversion; DesktopInput::ime only
calls it and touches no translator state. No callback/thread/atomic/queue exists
in this function path; the actual OS event loop and IME service remain separate.

The wrapper extension also extracts the actual DesktopInput storage and ime
method body, preserving the original BTreeSet/Instant fields. Verus treats only
std::time::Instant's representation as an opaque external dependency type; no
clock method, own runtime function, or business predicate is assumed. This is an
explicit dependency trust boundary, not an external_body on Jasper logic. The
wrapper contract preserves the entire translator state. Native/property/Miri/fuzz
and Kani fixture constructs the actual storage with Instant::now, then calls the
actual wrapper; OS clock support must be checked, never silently stubbed away.

Normalized-preedit pipeline slice: exact production normalize_ime output feeds
UiCompositionSlot::admit_normalized_preedit, retaining the full normalized event
borrow and refusing non-Preedit variants, stale serial/owner, and invalid local
UTF-8 bounds. The new Kani selection is explicitly --harness
normalized_preedit_pipeline (three fixed texts, arbitrary usize/None, unwind8 with
assertions). It exercises real ResourceGate/UiInputBinding close and composition
restart. The prior desktop_ime_coordinate_domain harness that constructs a
stateful DesktopInput still reaches unsupported clock_gettime and remains FAILED
in evidence/desktop-ime-chain. It is not claimed passed, waived or silently
excluded from that package's acceptance. This independently scoped pure host
conversion/admission path never instantiates DesktopInput and is also a real
production API for hosts without a desktop pointer clock. Native integration uses
actual winit Ime plus Scene, KeyboardRouter and owner binding, including empty
preedit clearing intent. This still does not mutate editor text or pixels.
Loom N/A for normalized admission: immutable event and binding/slot borrows only;
no locks, threads, atomics, asynchronous tasks or callbacks in these functions.

Owned preedit view: extracts UI PREEDIT VIEW KERNEL, including actual owned String
storage, new/current/apply/clear. Properties check moved text/cursor, reject-frame,
same-focus new serial, old clear refusal, and clear after binding close using the
actual normalizer, gate and slot. Kani explicit owned_preedit_view_lifecycle uses
three literal input texts, arbitrary usize/presence, plus fixed successor Chinese
text, unwind8 with assertions. It never constructs a stateful desktop clock; that
separate previously failing harness remains pending. Native desktop-feature root
tests use real winit + Scene/KeyboardRouter. This state store is not a document
engine or native/pixel renderer. current() is a host inspection borrow, not an
external authorization boundary; host owns one isolated view per target and must
clear it on cancellation/closure before rendering subsequent frames.
Loom N/A: UiPreeditView owns its Option/String and changes it only through &mut;
returned frame references borrow the view. No shared mutable state, thread, lock,
atomics, callbacks or queues exist in this production module path.

Paired preedit cancellation extracts UI PREEDIT CANCEL KERNEL. Actual slot.cancel
must succeed before view.clear is invoked. Rejection preserves both objects;
success clears only the matching view token and preserves any mismatched frame.
This supports cleanup after binding closure. Properties/Miri/fuzz run actual
normalizer -> apply -> paired cancel -> restart -> stale cancel -> close -> paired
cleanup. Kani explicitly selects paired_preedit_cancel_lifecycle, three input text
literals and arbitrary mismatch flag, unwind8 with assertions. The mismatch case
seeds an actual private UiPreeditFrame token with a different serial to verify the
specified defensive frame behavior, not substitute storage/algorithm. Arbitrary
slot/view states are covered by the Verus contract. Existing clock harness remains
pending outside this slice and is not silently counted passed.
Loom N/A: this pair holds &mut to both actual local objects for its synchronous
call, with no callback/queue/atomics/threads/locks. It is not an IPC transaction or
an acknowledgement of external/native cancellation or rendered-pixel clearing.

Focus/preedit close: extracts KEYBOARD PREEDIT CLOSE KERNEL and invokes actual
close_input_focus + cancel_preedit. Preflight requires an active composition;
when none exists hosts use ordinary close_input_focus. Four actual local objects
are held exclusively: router/binding/slot/view. Reject preserves all four;
success closes focus/binding, cancels slot and clears only the matching owned
frame, returning full old token as external cancellation intent. Ignoring the
helper bool is justified by the proved preflight and unchanged slot/caller
through normal focus closure, not an assumed success or rollback shortcut.
Kani explicitly selects focus_preedit_close_lifecycle: three text literals,
actual two-node focus Vec, caller/document mismatch, focus mismatch, close/reopen
and stale close; unwind8 retains assertions. Arbitrary states/order lengths are
Verus scope. Properties/Miri/fuzz retain previous drivers and exercise this path.
Loom N/A: exclusive synchronous four-object operation, no callbacks/locks/tasks/
threads/atomics/queue. External native cancel delivery, acknowledgements and pixel
updates are separate. The unsupported clock harness remains separately pending.

Lifecycle dispatch: actual normalizer now emits ImeEnabled(true/false), preserving
upstream availability events instead of dropping them. Actual
handle_preedit_lifecycle distinguishes Focus(false) (close active focus/binding/
slot/view) from IME disabled (cancel slot/view, keep focus/binding for ordinary
keyboard text). Enabled/focused/other events and invalid owners preserve all state.
Tests exercise actual normalizer and dispatcher; native root integration also
uses actual DesktopInput::translate with winit WindowEvent, never a copied mapper.
Kani explicitly selects preedit_host_lifecycle: three input literals, actual
single-node focus Vec, wrong session, enabled/no-op, disabled/restart/stale-disabled,
and focus-loss close; unwind8 retains assertions. No stateful desktop clock is
constructed in that bounded harness; the old unsupported wrapper remains pending.
Loom N/A for this path: pure conversion plus synchronous exclusive UI object
mutation, no async task/atomics/threads/locks/queue/callback. Returning a token is
native cancel intent, not an external acknowledgement. This handler requires an
active composition token; no-composition focus closure remains ordinary host logic.

Unified lifecycle extracts COMPOSITION CURRENT TOKEN (exact read-only correlation,
not permission) and INPUT LIFECYCLE DISPATCH. handle_input_lifecycle authenticates
the supplied current ticket before selecting idle/composing paths, so an old focus
close cannot borrow the new composition's token. Idle focus loss closes ordinary
input and returns (old ticket,None); active events delegate the proved preedit
handler and return the optional composition cancellation correlation. An idle slot
with orphaned presentation rejects unchanged, requiring explicit qualified cleanup.
Properties/Miri/fuzz exercise actual normalizer, router, gate/binding/slot/view,
ordinary idle loss, new-owner old-ticket refusal, disabled vs loss, and a real
legacy unpaired-cancel orphan followed by explicit cleanup. Kani explicitly selects
unified_input_lifecycle: three input literals, arbitrary disabled/lost choice,
actual single-entry focus Vec, reopen and orphan path, unwind8 with assertions.
Native root uses real winit WindowEvent through actual DesktopInput::translate.
The old clock harness remains pending, not claimed passed by this selection.
Loom N/A: synchronous exclusive mutation of four local UI objects; metadata getter
is read-only. No thread/atomics/lock/queue/callback in this production function path.
