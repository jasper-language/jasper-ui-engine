#[cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::semantics::Role;
use jasper_ui_engine::ui_ir::{
    MappingIssueKind, NativeUiDocument, NativeUiFrameError, SupportLevel, UiEventContext,
    UiEventKind, UiFrame, UiKey, UiPermission, UiPermissionContext,
};
use std::path::PathBuf;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ui-ir-v0")
        .join(name);
    std::fs::read(path).unwrap()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[test]
fn canonical_fixtures_are_pinned_to_the_appfw_schema_copy() {
    let expected = [
        ("data-table.ir.json", 0x001c_1eed_7126_6c7b),
        ("dialog-overlay.ir.json", 0x9087_2413_1f0e_7fac),
        ("settings-page.ir.json", 0xf909_ce32_c64a_e6b3),
    ];
    for (name, hash) in expected {
        let bytes = fixture(name);
        assert_eq!(fnv1a(&bytes), hash, "fixture drift: {name}");
        UiFrame::from_json_slice(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[test]
fn duplicate_and_zero_generations_are_rejected_before_native_mapping() {
    let bytes = fixture("settings-page.ir.json");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["ops"][0]["node"]["children"][1]["key"]["generation"] = serde_json::json!(0);
    let error = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert!(error
        .to_string()
        .contains("generation must be at least one"));

    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["ops"][0]["node"]["children"][1]["key"] =
        value["ops"][0]["node"]["children"][0]["key"].clone();
    let error = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert!(error.to_string().contains("duplicate inserted identity"));
}

#[test]
fn unknown_ir_fields_are_not_silently_ignored() {
    let bytes = fixture("settings-page.ir.json");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["ops"][0]["node"]["future_property"] = serde_json::json!(true);
    assert!(UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());

    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["ops"][0]["future_op_field"] = serde_json::json!(true);
    assert!(UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn dialog_fixture_retains_candidate_overlay_and_position_status() {
    let document = NativeUiDocument::from_json_slice(&fixture("dialog-overlay.ir.json")).unwrap();
    assert_eq!(document.revision(), 2);
    assert_eq!(document.scene().version(), 2);
    let overlay = document.mapped_properties()["overlay.layer"];
    assert_eq!(overlay.support, SupportLevel::Candidate);
    assert!(!overlay.host_accepted);
    let position = document.mapped_properties()["position.kind"];
    assert_eq!(position.support, SupportLevel::Candidate);
    assert!(!position.host_accepted);
    let dialog = document.node_for_key(UiKey::new(32, 1)).unwrap();
    assert!(document
        .scene()
        .find(dialog)
        .unwrap()
        .style
        .overlay
        .is_some());
}

#[test]
fn table_semantics_reject_instead_of_claiming_lossy_native_equivalence() {
    let error = NativeUiDocument::from_json_slice(&fixture("data-table.ir.json")).unwrap_err();
    let NativeUiFrameError::Mapping(issue) = error else {
        panic!("expected a structured mapping issue, got {error:?}");
    };
    assert_eq!(issue.kind, MappingIssueKind::InformationLoss);
    assert!(issue.path.ends_with(".semantics.role"));
}

#[test]
fn settings_page_event_identity_requires_current_owner_session_revision_and_generation() {
    let document = NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
    let button = UiKey::new(4, 1);
    let field = UiKey::new(3, 1);
    let context = UiEventContext {
        target: button,
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "host-input-policy",
            [UiPermission::Activate, UiPermission::TextEdit],
        ),
    };
    let admitted = document.admit_event(&context, UiEventKind::Click).unwrap();
    assert_eq!(admitted.action, "save");
    assert_eq!(
        document.node_for_key(button),
        Some(document.scene().root.children[2].id)
    );

    let mut stale = context.clone();
    stale.target = UiKey::new(4, 2);
    assert!(document.admit_event(&stale, UiEventKind::Click).is_err());

    let mut stale = context.clone();
    stale.frame_revision = 1;
    assert!(document.admit_event(&stale, UiEventKind::Click).is_err());

    let mut wrong_owner = context.clone();
    wrong_owner.owner = "another-host".into();
    assert!(document
        .admit_event(&wrong_owner, UiEventKind::Click)
        .is_err());

    let mut wrong_session = context.clone();
    wrong_session.session = "another-session".into();
    assert!(document
        .admit_event(&wrong_session, UiEventKind::Click)
        .is_err());

    let mut wrong_domain = context.clone();
    wrong_domain.identity_domain = "another-app/window".into();
    assert!(document
        .admit_event(&wrong_domain, UiEventKind::Click)
        .is_err());

    let no_text_edit = UiEventContext {
        target: field,
        permission_context: UiPermissionContext::new("pointer-only", [UiPermission::Pointer]),
        ..context
    };
    assert!(document
        .admit_event(&no_text_edit, UiEventKind::TextCommit)
        .is_err());
}

#[test]
fn successor_frames_update_move_remove_and_advance_atomically() {
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    frame["expected_revision"] = serde_json::json!(2);
    frame["next_revision"] = serde_json::json!(3);
    frame["ops"] = serde_json::json!([
        {
            "op": "update",
            "key": { "semantic_id": 3, "generation": 1 },
            "fields": {
                "semantics": {
                    "role": "text-field",
                    "label": "Display name",
                    "focusable": true
                },
                "events": [{ "event": "text-commit", "action": "rename" }]
            }
        },
        {
            "op": "move",
            "key": { "semantic_id": 4, "generation": 1 },
            "parent": { "semantic_id": 1, "generation": 1 },
            "index": 1
        },
        {
            "op": "env",
            "env": { "appearance": "dark", "reduce_motion": true, "text_scale": 1.25 }
        }
    ]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
    document.apply_frame(&frame).unwrap();
    assert_eq!(document.revision(), 3);
    assert_eq!(document.scene().version(), 3);
    assert_eq!(
        document.environment().appearance,
        Some(jasper_ui_engine::ui_ir::Appearance::Dark)
    );
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert_eq!(
        document.scene().find(field).unwrap().semantics.label,
        "Display name"
    );
    assert_eq!(
        document.scene().root.children[1].id,
        document.node_for_key(UiKey::new(4, 1)).unwrap()
    );

    let before_rejected = document.revision();
    let mut stale: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    stale["expected_revision"] = serde_json::json!(2);
    stale["next_revision"] = serde_json::json!(3);
    stale["ops"] = serde_json::json!([{
        "op": "remove",
        "key": { "semantic_id": 2, "generation": 1 }
    }]);
    let stale = UiFrame::from_json_slice(&serde_json::to_vec(&stale).unwrap()).unwrap();
    assert!(document.apply_frame(&stale).is_err());
    assert_eq!(document.revision(), before_rejected);
    assert!(document.node_for_key(UiKey::new(2, 1)).is_some());

    let mut remove: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    remove["expected_revision"] = serde_json::json!(3);
    remove["next_revision"] = serde_json::json!(4);
    remove["ops"] = serde_json::json!([{
        "op": "remove",
        "key": { "semantic_id": 2, "generation": 1 }
    }]);
    let remove = UiFrame::from_json_slice(&serde_json::to_vec(&remove).unwrap()).unwrap();
    document.apply_frame(&remove).unwrap();
    assert_eq!(document.revision(), 4);
    assert!(document.node_for_key(UiKey::new(2, 1)).is_none());
}

#[test]
fn resource_generations_reject_prepare_commit_release_misuse_and_reuse() {
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let operate = |revision: u64, ops: serde_json::Value| {
        let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        frame["expected_revision"] = serde_json::json!(revision);
        frame["next_revision"] = serde_json::json!(revision + 1);
        frame["ops"] = ops;
        UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap()
    };

    document
        .apply_frame(&operate(
            document.revision(),
            serde_json::json!([{
                "op": "resource",
                "resource": { "id": "logo", "generation": 1, "kind": "image" },
                "admission": "prepare"
            }]),
        ))
        .unwrap();
    document
        .apply_frame(&operate(
            document.revision(),
            serde_json::json!([{
                "op": "resource",
                "resource": { "id": "logo", "generation": 1, "kind": "image" },
                "admission": "commit"
            }]),
        ))
        .unwrap();
    document
        .apply_frame(&operate(
            document.revision(),
            serde_json::json!([{
                "op": "resource",
                "resource": { "id": "logo", "generation": 1, "kind": "image" },
                "admission": "release"
            }]),
        ))
        .unwrap();
    let revision = document.revision();
    assert!(document
        .apply_frame(&operate(
            document.revision(),
            serde_json::json!([{
                "op": "resource",
                "resource": { "id": "logo", "generation": 1, "kind": "image" },
                "admission": "prepare"
            }])
        ))
        .is_err());
    assert_eq!(document.revision(), revision);
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn same_settings_fixture_executes_through_scene_host_view_and_cpu_renderer() {
    use jasper_ui_engine::native::host::SceneHostView;

    const OWNER: u64 = 71;
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 320;
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let document = NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
    let mut view = SceneHostView::new(OWNER, document.scene(), &font, WIDTH, HEIGHT)
        .expect("Native scene construction");
    let version = document.revision();

    let root = document.node_for_key(UiKey::new(1, 1)).unwrap();
    let heading = document.node_for_key(UiKey::new(2, 1)).unwrap();
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    let button = document.node_for_key(UiKey::new(4, 1)).unwrap();
    let root_bounds = view.node_bounds(OWNER, version, root).unwrap();
    assert!((root_bounds.width() - WIDTH as f32).abs() < 0.5);
    assert!(view.node_bounds(OWNER, version, heading).is_some());
    assert!(view.node_bounds(OWNER, version, field).is_some());
    assert!(view.node_bounds(OWNER, version, button).is_some());

    let semantics = view.semantics(OWNER, version).unwrap();
    let role = |node| {
        semantics
            .nodes
            .iter()
            .find(|item| item.node == node)
            .map(|item| item.semantics.role)
            .unwrap()
    };
    assert_eq!(
        role(heading),
        Role::Heading(jasper_ui_engine::semantics::HeadingLevel::new(1).unwrap())
    );
    assert_eq!(role(field), Role::TextField);
    assert_eq!(role(button), Role::Button);

    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    assert!(view.render_to_surface(
        OWNER,
        version,
        &mut pixels,
        (WIDTH * 4) as usize,
        48 * 1024 * 1024,
    ));
    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel.iter().any(|byte| *byte != 0)),
        "the canonical settings fixture must produce visible pixels"
    );
}
