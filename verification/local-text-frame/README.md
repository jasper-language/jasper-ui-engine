# Local text frame — acceptance pending

Own production source: `src/text_skia.rs`, opt-in `skia-text-pending` only.
Native integration runs are preliminary diagnostics, not stages of the ordered gate.
All six acceptance stages remain incomplete. Do not mark this module delivered.

## Precise obligations / exact implementation mapping

- LTF-01, `LocalTextRenderer::prepare`: on success, width/height are nonzero and
  representable as native signed dimensions; row_bytes = 4*width;
  byte_len = row_bytes*height <= caller budget, with no arithmetic overflow.
  Calls the existing actual `rgba8_row_bytes` and `surface_span`; their scoped
  proofs do not prove this caller, allocation success or native rendering.
- LTF-02, `LocalTextFrame<'a>` + `prepare`: source is the exact borrowed Scene
  used for layout, and its immutable borrow spans result consumption. No version
  equality shortcut or cross-frame cache. Current negative compile fixture covers
  root mutation, but Rust type checking is not implementation-level Verus proof.
- LTF-03, `prepare` + `into_rgba`: exactly one prepared Paragraph for each nonempty
  laid-out text op, in paint traversal order; canvas text rejected. Each Paragraph
  uses its final content width, font size/color and injected font collection.
  Prove queue correspondence/clip balance, not merely matching counts.
- LTF-04, native side effects: output surface private until complete readback;
  error publishes no pixel vector. No claim to roll back native allocations or
  provide atomic IPC presentation. Allocation failure/abort is outside Result's
  recoverable-error guarantee and must remain explicit.
- LTF-05, renderer/font lifetime: injected bytes copied, no system-font fallback,
  Paragraph retains native ownership after renderer drop. Own Rust ownership and
  API call sequence need proof; native reference counting, parser, Unicode,
  shaping, float layout and rasterization remain unproved dependencies.
- LTF-06, `measure`/`prepare`: invalid measurement inputs return empty legacy
  metrics; prepare rejects invalid font sizes, nonfinite final geometry, unresolved
  glyphs, or final paragraph/content height mismatch > 0.1. This is a stated
  compatibility restriction, not universal text-layout equivalence.

## Ordered gate, not yet passed

1. Verus+Z3: implementation proof harness and native boundary specifications pending.
   Do not external_body the new Rust adapter to manufacture a pass.
2. Proptest: not started; native fixture is not the property suite.
3. Miri: not started; native FFI support must be established, never silently skipped.
4. cargo-fuzz: not started; font/native input limits and meaningful corpus required.
5. Kani: not started; retain unsupported native/float diagnostics honestly if encountered.
6. Loom: applicability review pending; no invented concurrency model. Intended first
   path is synchronous, but actual native handle Send/Sync boundaries must be audited.

Native tests: tests/local_text_frame.rs. Supply explicit font paths using
JASPER_UI_TEST_FONT and JASPER_UI_TEST_CJK_FONT in the resource-limited VM only.
Existing default renderer, UiBatch wire format and OS integration are unchanged.


## 2026-10-24 19:02 UTC trigger: actual storage proof progress

The production prepare call now uses extracted text_frame_storage. Actual bodies
of it and the two old surface helpers have Verus 3/0 + two rejected semantic
negative controls, Proptest (3 tests, 256 cases/property), Miri (7 boundaries),
and seeded fuzz (4,431,914 runs/11s) evidence. Kani's unchanged independent u128
oracle did not finish under the capped command, first CaDiCaL then one Z3 retry;
both exit124 outcomes are retained. No further solver retries this round.
Loom stage not reached; scalar applicability rationale does not cover native handles.
Full renderer and even this ordered scalar chain remain unaccepted. No formal
proof of prepare ownership/paragraph queue/FFI is inferred from this subroutine.
See evidence/local-text-storage-chain/report.json and source-snapshot.zip.


## 2026-10-24 19:15 UTC trigger: own renderer hardening, still unaccepted

- LTF-07: valid_draw_op and finite_rect must reject nonfinite coordinates, endpoint
  overflow and negative/nonfinite widths before surface creation. Zero width must
  be omitted rather than Skia hairline semantics. Own guards remain unproved.
- LTF-08: private PhantomData<Rc<()>> on both actual public own types restricts safe
  Send/Sync transfer. Actual compiler negative fixtures supplement the pending
  ownership/formal mapping. Native internals are still third-party TCB; do not
  infer complete Loom acceptance from a marker or an empty concurrency model.

VM preliminary checks: 9 native tests and 3 borrow/thread compile-fail fixtures.
Removing both zero-width guards in VM makes the pixel test fail; restoring exact
source passes it. No new Canvas feature or system integration was added. The
original scalar three-function proof bodies remain byte-identical, with machine
mapping in evidence/local-text-frame-guard-pending/storage-proof-mapping.json;
new source outside those bodies remains unproved, and the prior Kani timeout is
unchanged. No solver rerun or stage waiver.


## 2026-10-24 19:25 UTC trigger: text geometry implementation, verification in progress

LTF-09: hit_test_text must select only the requested node in the frame's own
prepared text records, translate finite viewport coordinates to finite local
coordinates, reject negative/out-of-range native positions, and map affinity
without exposing native types. Success returns UTF-16 units only: no UTF-8 edit
position, grapheme-boundary proof or input authorization is inferred. Same native
Paragraph is queried and later painted. Full own-wrapper implementation proof
and six-stage evidence remain pending; new native fixtures were submitted to
the resource-capped VM and have no completed result yet. See
 evidence/local-text-hit-pending/run-state.json for the owned active session.
Do not start concurrent UI verification or replace this work with polling/docs.


## 2026-10-24 19:35 UTC trigger: VM failure collected, exact range proof staged

Owned native session55788 finished exit124 with only a compiler-start line, not
completed tests. A fresh bounded read-only SSH probe failed exit255 (server not
responding). No new heavy command is started while VM is unavailable, and no
host compiler fallback is permitted. Outer UI-wrapper wall-clock timeout now
bounds transport/setup/pull to300s; inner180s and resource caps unchanged.

Actual hit_test_text now calls marked text_hit_offset. Its proposed exact-source
Verus contract is: Some(i) iff native signed position is nonnegative and <=UTF16
length, and i equals that position; None iff it is negative or exceeds length.
Generated offset-proof-candidate.rs is NOT executed proof. Property/Miri/fuzz
and Kani hit-only harnesses are staged (do not rerun storage timeout harness).
Next run must start at Verus+Z3 and follow all stages, retaining the independent
old storage timeout. Full node lookup/affinity/paragraph/coordinate properties
remain separate pending obligations; no glyph/grapheme or edit claim is added.
