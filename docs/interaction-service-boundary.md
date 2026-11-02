# Interaction and external-service boundary

Updated: 2026-11-02

This document closes the public client boundary for item 4. It does not claim
that a live editor, operating-system IME, AT-SPI/UIA adapter or window system
has been supplied in this environment.

## Public client path

`native::host::TextInputSession` owns one focus slot and correlates:

- a `UiInputTicket` for the current target/document/revision;
- the newest `UiCompositionToken` for transient preedit presentation;
- local display focus and preedit cleanup;
- a FIFO of exact cancellation envelopes whose host transport failed.

The external text/IME service is injected through
`native::host::ExternalInputService`. A successful
`deliver_cancel` call means only that the host transport accepted the
`InputServiceCancellation`; it is not a service acknowledgement and does not
prove that native input work has stopped. Failed transport delivery is retained
and can be retried with the same owner and composition identities.

The UI consumes authoritative text only through
`TextDisplayView::snapshot` or `TextInputSession::replace_display`. A display
successor must keep the same target, target generation and document and must
advance its opaque revision. Equal, older and stale-expected snapshots are
rejected before display mutation. A successful successor rotates the focus
ticket, clears the old preedit correlation and makes late preedit/display
payloads for the old generation unusable. The UI does not construct an edit
transaction, own a second document, implement an IME, show its own candidates
or replay committed text.

Focus loss/deactivation/removal/retirement is ordered as follows:

1. reject new payloads for the old focus ticket;
2. clear native focus and matching preedit presentation;
3. request pointer-host release when a capture client is supplied;
4. deliver one cancellation envelope to the external input service;
5. retain a failed transport attempt for explicit retry.

A pointer release failure does not reopen UI focus. It remains retryable through
the capture client. Retirement permanently prevents reopening the
`TextInputSession`, even when external cancellation delivery is still pending.

## Accessibility boundary

`SceneHostView::accessibility_tree` returns a versioned host-neutral tree with
visible source nodes, roles, labels/descriptions/values, disabled/focus state,
view-local bounds and only the focus/activate actions admitted by the engine.
The AccessKit feature converts full and incremental trees to AccessKit
`TreeUpdate` values and admits exact current-version action requests.

The engine does not create a platform accessibility adapter, call AT-SPI/UIA,
or claim that a screen reader has consumed a tree. Those adapters and their
authentication/liveness policies belong to the host. Stale-version and
unsupported requests are rejected without dispatching focus or activation.

## Capability and TCB matrix

| Capability | Owner | Current boundary |
|---|---|---|
| Scene focus, keyboard/pointer routing, duplicate press suppression | Jasper UI | Host must authenticate the owner and supply normalized input |
| Text display projection and preedit text | Jasper UI | Display-only; no editor transaction or candidate authority |
| Authoritative text document and commits | External host/text service | Explicit TCB; only snapshots/selected display text cross the boundary |
| Native IME engine, candidate service and cancellation completion | External host/OS service | Explicit TCB; UI receives correlation and preedit presentation inputs |
| Pointer/window focus/capture grant | External host | Capture/release acknowledgement is injected and may fail/retry |
| Host-neutral accessibility tree | Jasper UI | Visibility, geometry and admitted actions; no OS adapter |
| AccessKit `TreeUpdate` conversion | Jasper UI optional feature | Serialization/action admission only; root/platform adapter remains host-owned |
| AT-SPI, UIA, system accessibility runtime | External platform | Not implemented or verified here |

The loopback test in `tests/native_interaction_closure.rs` uses the real public
types above. It substitutes only external service/window behaviour: an injected
font, an ACK-capable capture host and a cancellation transport. It verifies
failure and retry, late/duplicate preedit, authoritative successor rebinding,
focus/capture loss, retirement, scene deletion/new-target rebinding,
duplicate-up suppression and the exported accessibility tree. It is not a live
IME, editor or screen-reader integration test.

## Verification scope

Basic executable evidence is provided by:

```sh
JASPER_UI_TEST_FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  cargo test --locked --features native-blitz-pending \
  --test native_interaction_closure
```

More general refinement proofs for the external async delivery relation,
native service cancellation acknowledgement and platform accessibility
runtime are registered for later Claude proof work. They are not replaced by
the loopback tests. Native Blitz event dispatch, font shaping, capture transport
and AccessKit are third-party TCBs; the resource/liveness behavior of a live
external service also remains a host TCB.
