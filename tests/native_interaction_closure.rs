//! Public interaction/service closure through an injected loopback transport.
//! The test host supplies font, capture acknowledgement and cancellation transport;
//! it does not impersonate a live editor, IME or platform accessibility adapter.

#![cfg(feature = "native-blitz-pending")]

use jasper_ui_engine::native::host::{
    platform_bridge_capabilities, CancellationDelivery, ExternalInputService,
    InputCancellationReason, InputServiceCancellation, PlatformBridgeCapability,
    PointerCaptureHost, SceneHostView, TextDisplayView, TextInputSession, TextInputSessionError,
    TextPointerCapture, UiInputContext, UiInputTicket,
};
use jasper_ui_engine::native::{EventReply, SceneEventKind, SceneFocusEvent, SceneUpdate};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};

#[derive(Default)]
struct LoopbackInputService {
    requests: Vec<InputServiceCancellation>,
    unavailable: bool,
}

impl ExternalInputService for LoopbackInputService {
    fn deliver_cancel(&mut self, request: InputServiceCancellation) -> bool {
        self.requests.push(request);
        !self.unavailable
    }
}

#[derive(Default)]
struct LoopbackCaptureHost {
    captures: usize,
    releases: usize,
    deny_release: bool,
}

impl PointerCaptureHost for LoopbackCaptureHost {
    fn capture(&mut self, _: UiInputTicket) -> bool {
        self.captures += 1;
        true
    }
    fn release(&mut self, _: UiInputTicket) -> bool {
        self.releases += 1;
        !self.deny_release
    }
}

fn font() -> Vec<u8> {
    std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT").expect("explicit injected test font required"),
    )
    .expect("injected test font readable")
}

fn context(target: u64, revision: u64) -> UiInputContext {
    UiInputContext {
        target: NodeId(target),
        target_generation: 1,
        document: 20,
        revision,
    }
}

#[test]
fn composition_cancel_is_exact_pending_retry_and_late_payload_is_rejected() {
    let font = font();
    let initial = context(9, 3);
    let mut view = TextDisplayView::new(10, initial, &font, "External authority", 320, 180);
    let mut session = TextInputSession::new(10, 1);
    let mut service = LoopbackInputService::default();
    let mut focus_events = Vec::new();
    let owner = session
        .begin(&mut view, initial, &mut |event| focus_events.push(event))
        .unwrap();
    let token = session.begin_preedit(&mut view).unwrap();
    assert!(session.preedit(&mut view, token, Some("候选")));

    service.unavailable = true;
    let cancelled = session
        .cancel_preedit(
            &mut view,
            &mut service,
            InputCancellationReason::ImeDisabled,
        )
        .unwrap();
    assert!(cancelled.preedit_cleared);
    assert_eq!(cancelled.cancellation, CancellationDelivery::Pending);
    assert_eq!(session.pending_cancellations(), 1);
    assert_eq!(service.requests.len(), 1);
    let request = service.requests[0];
    assert_eq!(request.session, 10);
    assert_eq!(request.owner, owner);
    assert_eq!(request.composition, Some(token));
    assert_eq!(request.reason, InputCancellationReason::ImeDisabled);

    // Cancellation is idempotent at the owner boundary: a duplicate or late
    // preedit cannot enqueue another service request or resurrect the slot.
    assert!(session
        .cancel_preedit(
            &mut view,
            &mut service,
            InputCancellationReason::ImeDisabled
        )
        .is_none());
    assert!(!session.preedit(&mut view, token, Some("late")));
    assert_eq!(session.current_owner(), Some(owner));
    assert_eq!(service.requests.len(), 1);

    service.unavailable = false;
    assert_eq!(
        session.retry_cancel(&mut service),
        CancellationDelivery::Queued
    );
    assert_eq!(session.pending_cancellations(), 0);
    assert_eq!(
        session.retry_cancel(&mut service),
        CancellationDelivery::NoRequest
    );
    assert_eq!(service.requests.len(), 2);
    assert_eq!(service.requests[1], request);
    assert_eq!(focus_events.len(), 1);
}

#[test]
fn authoritative_snapshot_advances_once_and_invalidates_old_ticket_and_preedit() {
    let font = font();
    let first = context(9, 3);
    let second = UiInputContext {
        revision: 4,
        ..first
    };
    let old = "External authority v3";
    let mut view = TextDisplayView::new(10, first, &font, old, 320, 180);
    let mut session = TextInputSession::new(10, 1);
    let mut events = Vec::new();
    let owner = session
        .begin(&mut view, first, &mut |event| events.push(event))
        .unwrap();
    let old_token = session.begin_preedit(&mut view).unwrap();
    assert!(session.preedit(&mut view, old_token, Some("old")));

    let rebind = session
        .replace_display(&mut view, first, second, "External authority v4")
        .unwrap();
    assert_eq!(rebind.previous_owner, owner);
    assert_ne!(rebind.current_owner.key.generation, owner.key.generation);
    assert_eq!(rebind.cleared_composition, Some(old_token));
    assert_eq!(rebind.current_owner.context, second);
    assert_eq!(session.current_owner(), Some(rebind.current_owner));
    assert_eq!(session.current_composition(), None);
    assert_eq!(view.context_for_owner(10), Some(second));
    assert!(!session.preedit(&mut view, old_token, Some("late")));

    assert_eq!(
        session.replace_display(&mut view, first, second, "duplicate"),
        Err(TextInputSessionError::StaleContext)
    );
    assert_eq!(
        session.replace_display(&mut view, second, second, "duplicate"),
        Err(TextInputSessionError::StaleContext)
    );
    assert_eq!(
        session.replace_display(&mut view, second, first, "late"),
        Err(TextInputSessionError::StaleContext)
    );
    assert_eq!(view.context_for_owner(10), Some(second));
    assert!(!view.snapshot(10, second, second, "duplicate same revision"));
    assert!(!view.snapshot(10, second, first, "older revision"));
    assert_eq!(view.context_for_owner(10), Some(second));

    let fresh = session.begin_preedit(&mut view).unwrap();
    assert_ne!(fresh.serial, old_token.serial);
    assert!(session.preedit(&mut view, fresh, Some("successor")));
}

#[test]
fn focus_loss_closes_focus_before_recovering_capture_and_service_cancel() {
    let font = font();
    let initial = context(9, 3);
    let mut view = TextDisplayView::new(10, initial, &font, "Pointer and IME", 320, 180);
    let mut session = TextInputSession::new(10, 1);
    let mut service = LoopbackInputService::default();
    let mut host = LoopbackCaptureHost::default();
    let mut capture =
        TextPointerCapture::new(10, 2, |_| EventReply::default(), |_| EventReply::default());
    let owner = session.begin(&mut view, initial, &mut |_| {}).unwrap();
    let token = session.begin_preedit(&mut view).unwrap();
    assert!(session.preedit(&mut view, token, Some("组合")));
    let pointer = capture
        .down(&mut view, &mut host, 10, initial, 12.0, 12.0)
        .unwrap();
    assert!(capture.move_pointer(&mut view, 10, pointer, 40.0, 12.0));

    host.deny_release = true;
    service.unavailable = true;
    let lost = session
        .focus_lost_with_capture(
            &mut view,
            &mut capture,
            &mut host,
            &mut service,
            InputCancellationReason::FocusLost,
            &mut |_| {},
        )
        .unwrap();
    assert!(lost.focus_closed);
    assert!(lost.preedit_cleared);
    assert_eq!(lost.cancellation, CancellationDelivery::Pending);
    assert_eq!(session.current_owner(), None);
    assert_eq!(session.current_composition(), None);
    assert_eq!(service.requests.len(), 1);
    assert_eq!(service.requests[0].owner, owner);
    assert_eq!(service.requests[0].composition, Some(token));
    assert!(session
        .focus_lost_with_capture(
            &mut view,
            &mut capture,
            &mut host,
            &mut service,
            InputCancellationReason::FocusLost,
            &mut |_| {}
        )
        .is_none());
    assert_eq!(service.requests.len(), 1);

    host.deny_release = false;
    assert!(capture.retry_release(&mut host, 10));
    service.unavailable = false;
    assert_eq!(
        session.retry_cancel(&mut service),
        CancellationDelivery::Queued
    );
    assert_eq!(service.requests.len(), 2);
    assert_eq!(service.requests[1], service.requests[0]);
}

#[test]
fn retirement_blocks_reopen_and_keeps_failed_cancel_retryable() {
    let font = font();
    let initial = context(9, 3);
    let mut view = TextDisplayView::new(10, initial, &font, "Retiring display", 320, 180);
    let mut session = TextInputSession::new(10, 1);
    let mut service = LoopbackInputService {
        unavailable: true,
        ..Default::default()
    };
    let owner = session.begin(&mut view, initial, &mut |_| {}).unwrap();
    let token = session.begin_preedit(&mut view).unwrap();
    assert!(session.preedit(&mut view, token, Some("retiring")));
    let retired = session
        .retire(
            &mut view,
            &mut service,
            InputCancellationReason::Retired,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(retired.cancellation, CancellationDelivery::Pending);
    assert_eq!(service.requests.len(), 1);
    assert_eq!(service.requests[0].owner, owner);
    assert_eq!(service.requests[0].composition, Some(token));
    assert_eq!(view.context_for_owner(10), None);
    assert_eq!(
        session.begin(&mut view, initial, &mut |_| {}),
        Err(TextInputSessionError::Retired)
    );
    assert!(session
        .retire(
            &mut view,
            &mut service,
            InputCancellationReason::Retired,
            &mut |_| {}
        )
        .is_none());
    assert_eq!(service.requests.len(), 1);
    service.unavailable = false;
    assert_eq!(
        session.retry_cancel(&mut service),
        CancellationDelivery::Queued
    );
    assert_eq!(service.requests.len(), 2);
    assert_eq!(service.requests[0], service.requests[1]);
}

#[test]
fn capture_retirement_closes_local_view_before_release_retry() {
    let font = font();
    let initial = context(9, 3);
    let mut view = TextDisplayView::new(10, initial, &font, "Capture retirement", 320, 180);
    let mut session = TextInputSession::new(10, 1);
    let mut service = LoopbackInputService::default();
    let mut host = LoopbackCaptureHost::default();
    let mut capture =
        TextPointerCapture::new(10, 2, |_| EventReply::default(), |_| EventReply::default());
    let owner = session.begin(&mut view, initial, &mut |_| {}).unwrap();
    let token = session.begin_preedit(&mut view).unwrap();
    assert!(session.preedit(&mut view, token, Some("组合")));
    capture
        .down(&mut view, &mut host, 10, initial, 12.0, 12.0)
        .unwrap();
    host.deny_release = true;
    service.unavailable = true;
    let retired = session
        .retire_with_capture(
            &mut view,
            &mut capture,
            &mut host,
            &mut service,
            InputCancellationReason::Retired,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(retired.cancellation, CancellationDelivery::Pending);
    assert_eq!(service.requests[0].owner, owner);
    assert_eq!(service.requests[0].composition, Some(token));
    assert_eq!(session.current_owner(), None);
    assert_eq!(view.context_for_owner(10), None);
    assert_eq!(
        session.begin(&mut view, initial, &mut |_| {}),
        Err(TextInputSessionError::Retired)
    );
    host.deny_release = false;
    assert!(capture.retry_release(&mut host, 10));
    service.unavailable = false;
    assert_eq!(
        session.retry_cancel(&mut service),
        CancellationDelivery::Queued
    );
    assert_eq!(service.requests.len(), 2);
    assert_eq!(service.requests[0], service.requests[1]);
}

#[test]
fn scene_delete_reports_focus_loss_and_new_target_rebinds_with_fresh_accessibility() {
    let font = font();
    let capabilities = platform_bridge_capabilities();
    assert_eq!(
        capabilities.text_display_projection,
        PlatformBridgeCapability::EngineProvided
    );
    assert_eq!(
        capabilities.authoritative_text_document,
        PlatformBridgeCapability::ExternalHostOwned
    );
    assert_eq!(
        capabilities.native_ime_engine,
        PlatformBridgeCapability::ExternalHostOwned
    );
    assert_eq!(
        capabilities.platform_accessibility_adapter,
        PlatformBridgeCapability::ExternalHostOwned
    );
    let old = Node::text(NodeId(2), "Old field").with_semantics(
        Semantics::default()
            .role(Role::TextField)
            .label("Old field")
            .focusable(true),
    );
    let mut scene = Scene::new(Node::container(NodeId(1)).child(old));
    let mut scene_view = SceneHostView::new(10, &scene, &font, 320, 180).unwrap();
    let old_version = scene.version();
    assert!(scene_view.focus(10, old_version, NodeId(2), &mut |_| {}));
    let tree = scene_view.accessibility_tree(10, old_version).unwrap();
    assert_eq!(tree.focused, Some(NodeId(2)));

    let display_context = context(2, 3);
    let mut display = TextDisplayView::new(10, display_context, &font, "Old field", 320, 180);
    let mut session = TextInputSession::new(10, 1);
    let mut service = LoopbackInputService::default();
    session
        .begin(&mut display, display_context, &mut |_| {})
        .unwrap();
    let token = session.begin_preedit(&mut display).unwrap();
    assert!(session.preedit(&mut display, token, Some("old composition")));

    let new = Node::text(NodeId(3), "New field").with_semantics(
        Semantics::default()
            .role(Role::TextField)
            .label("New field")
            .focusable(true),
    );
    let patches = [
        ScenePatch::Remove { id: NodeId(2) },
        ScenePatch::Insert {
            parent: NodeId(1),
            index: 0,
            node: new,
        },
    ];
    scene.apply_batch_at(old_version, &patches).unwrap();
    let mut focus_events = Vec::new();
    assert!(scene_view.apply_with_focus_events(
        10,
        old_version,
        scene.version(),
        &patches,
        &mut |event| focus_events.push(event)
    ));
    assert_eq!(focus_events.len(), 1);
    assert!(!focus_events[0].focused);
    assert_eq!(focus_events[0].node, NodeId(2));
    assert!(!scene_view.activate(10, scene.version(), NodeId(2), &mut |_| {}));
    let stale_focus = SceneFocusEvent {
        scene_version: old_version,
        node: NodeId(3),
        focused: false,
    };
    assert!(session
        .scene_focus_lost(
            &mut display,
            &mut service,
            &stale_focus,
            InputCancellationReason::TargetRemoved,
            &mut |_| {}
        )
        .is_none());
    assert!(service.requests.is_empty());

    let lost = session
        .scene_focus_lost(
            &mut display,
            &mut service,
            &focus_events[0],
            InputCancellationReason::TargetRemoved,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(lost.cancellation, CancellationDelivery::Queued);
    assert!(!session.preedit(&mut display, token, Some("late old composition")));

    assert!(scene_view.focus(10, scene.version(), NodeId(3), &mut |_| {}));
    let tree = scene_view.accessibility_tree(10, scene.version()).unwrap();
    assert!(tree
        .nodes
        .iter()
        .any(|node| node.node == NodeId(3) && node.actions.focus && node.role == Role::TextField));
    assert!(!tree.nodes.iter().any(|node| node.node == NodeId(2)));
    assert_eq!(tree.focused, Some(NodeId(3)));

    let next = context(3, 1);
    let mut replacement = TextDisplayView::new(10, next, &font, "New field", 320, 180);
    let owner = session.begin(&mut replacement, next, &mut |_| {}).unwrap();
    assert_eq!(owner.context, next);
    assert_ne!(
        owner.key.generation,
        service.requests[0].owner.key.generation
    );
    assert!(session
        .preedit(&mut display, token, Some("late old target"))
        .eq(&false));
}

#[test]
fn scene_pointer_and_keyboard_duplicates_do_not_repeat_activation() {
    use jasper_ui_engine::native::{HostKeyInput, KeyLocation, PointerAction};
    let font = font();
    let button = Node::text(NodeId(2), "Save").with_semantics(
        Semantics::default()
            .role(Role::Button)
            .label("Save")
            .focusable(true),
    );
    let mut scene = Scene::new(Node::container(NodeId(1)).child(button));
    let mut view = SceneHostView::new(10, &scene, &font, 320, 180).unwrap();
    let mut activations = 0;
    for (phase, pressed) in [
        (PointerAction::Down, true),
        (PointerAction::Up, false),
        (PointerAction::Up, false),
    ] {
        assert!(view.pointer(
            10,
            scene.version(),
            phase,
            12.0,
            12.0,
            pressed,
            &mut |event| {
                if event.kind == SceneEventKind::Activate {
                    activations += 1;
                }
                EventReply::default()
            },
            &mut |_| {}
        ));
    }
    assert_eq!(activations, 1);
    assert!(view.focus(10, scene.version(), NodeId(2), &mut |_| {}));
    let key = HostKeyInput {
        key: "Enter",
        code: "Enter",
        pressed: true,
        repeat: false,
        composing: false,
        modifiers: Default::default(),
        location: KeyLocation::Standard,
    };
    let mut commands = 0;
    for _ in 0..2 {
        assert!(view.key(
            10,
            scene.version(),
            &key,
            &mut |_| {
                commands += 1;
                EventReply::default()
            },
            &mut |_| {}
        ));
    }
    assert_eq!(commands, 2);
    assert_eq!(
        activations, 1,
        "key routing must not duplicate pointer activation"
    );
    assert!(view.deactivate(10, scene.version(), &mut |_| {}));
    assert_eq!(view.focused_node(10, scene.version()), None);
    let _ = view.pointer(
        10,
        scene.version(),
        PointerAction::Up,
        12.0,
        12.0,
        false,
        &mut |event| {
            if event.kind == SceneEventKind::Activate {
                activations += 1;
            }
            EventReply::default()
        },
        &mut |_| {},
    );
    assert_eq!(activations, 1);

    let old = scene.version();
    let patch = [ScenePatch::UpdateStyle {
        id: NodeId(2),
        style: jasper_ui_engine::style::Style::default()
            .background(jasper_ui_engine::style::Color::BLUE),
    }];
    scene.apply_batch_at(old, &patch).unwrap();
    assert!(view.apply_update(
        10,
        SceneUpdate {
            expected_version: old,
            next_version: scene.version(),
            patches: &patch
        }
    ));
    assert!(view.close(10));
    assert!(!view.deactivate(10, scene.version(), &mut |_| {}));
}
