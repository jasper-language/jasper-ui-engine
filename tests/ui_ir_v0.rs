use jasper_ui_engine::semantics::Role;
use jasper_ui_engine::ui_ir::{
    MappingIssueKind, NativeActivationSource, NativeUiDocument, NativeUiFrameError, ResourceKind,
    SupportLevel, UiEventContext, UiEventKind, UiFrame, UiKey, UiPermission, UiPermissionContext,
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
        let frame =
            UiFrame::from_json_slice(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let encoded = frame.to_json_vec().unwrap();
        assert_eq!(
            UiFrame::from_json_slice(&encoded).unwrap(),
            frame,
            "typed IR round-trip: {name}"
        );
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
fn unprojected_content_and_kind_changes_fail_without_advancing_revision() {
    let bytes = fixture("settings-page.ir.json");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["ops"][0]["node"]["element"]["content"] = serde_json::json!("not representable");
    let error =
        NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    let NativeUiFrameError::Mapping(issue) = error else {
        panic!("expected a structured mapping issue");
    };
    assert_eq!(issue.kind, MappingIssueKind::InformationLoss);
    assert!(issue.path.ends_with(".element.content"));

    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let before = document.scene().clone();
    let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    frame["expected_revision"] = serde_json::json!(2);
    frame["next_revision"] = serde_json::json!(3);
    frame["ops"] = serde_json::json!([{
        "op": "update",
        "key": { "semantic_id": 3, "generation": 1 },
        "fields": { "element": { "kind": "container" } }
    }]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
    let error = document.apply_frame_with_patches(&frame).unwrap_err();
    let NativeUiFrameError::Mapping(issue) = error else {
        panic!("expected a structured kind-projection issue");
    };
    assert_eq!(issue.kind, MappingIssueKind::InformationLoss);
    assert!(issue.path.ends_with(".fields.element.kind"));
    assert_eq!(document.revision(), 2);
    assert_eq!(document.scene(), &before);
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
fn table_semantics_are_preserved_in_the_native_scene() {
    let document = NativeUiDocument::from_json_slice(&fixture("data-table.ir.json")).unwrap();
    let role = |key| {
        document
            .scene()
            .find(document.node_for_key(UiKey::new(key, 1)).unwrap())
            .unwrap()
            .semantics
            .role
    };
    assert_eq!(role(20), Role::Table);
    assert_eq!(role(21), Role::TableRow);
    assert_eq!(role(22), Role::TableCell);
    assert_eq!(role(23), Role::TableCell);
}

#[test]
fn text_area_and_popup_roles_keep_their_schema_identity() {
    let bytes = fixture("settings-page.ir.json");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["ops"][0]["node"]["children"][1]["element"]["kind"] = serde_json::json!("text-area");
    value["ops"][0]["node"]["children"][1]["semantics"]["role"] = serde_json::json!("text-area");
    let document = NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert_eq!(
        document.scene().find(field).unwrap().semantics.role,
        Role::TextArea
    );

    value["ops"][0]["node"]["children"][1]["semantics"]["selected"] = serde_json::json!(true);
    let document = NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert!(document.scene().find(field).unwrap().semantics.selected);

    value["ops"][0]["node"]["children"][1]["element"] = serde_json::json!({ "kind": "popover" });
    value["ops"][0]["node"]["children"][1]["semantics"]["role"] = serde_json::json!("popover");
    let document = NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert_eq!(
        document.scene().find(field).unwrap().semantics.role,
        Role::Popover
    );

    value["ops"][0]["node"]["children"][1]["element"] =
        serde_json::json!({ "kind": "tooltip", "content": "Help" });
    value["ops"][0]["node"]["children"][1]["semantics"]["role"] = serde_json::json!("tooltip");
    let document = NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert_eq!(
        document.scene().find(field).unwrap().semantics.role,
        Role::Tooltip
    );

    value["ops"][0]["node"]["children"][1]["element"] = serde_json::json!({ "kind": "container" });
    value["ops"][0]["node"]["children"][1]["semantics"]["role"] = serde_json::json!("live");
    let document = NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert_eq!(
        document.scene().find(field).unwrap().semantics.role,
        Role::Live(jasper_ui_engine::semantics::LivePoliteness::Polite)
    );
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
    let button_node = document.node_for_key(button).unwrap();
    assert_eq!(
        document
            .admit_activation(&context, button_node, NativeActivationSource::PointerClick)
            .unwrap()
            .action,
        "save"
    );
    assert!(
        document
            .admit_activation(&context, button_node, NativeActivationSource::Keyboard)
            .is_err(),
        "a click binding does not silently accept keyboard activate"
    );
    assert!(document
        .admit_native_target(&context, button_node, UiPermission::Focus)
        .is_err());
    let focus = UiEventContext {
        permission_context: UiPermissionContext::new("tab-focus", [UiPermission::Focus]),
        ..context.clone()
    };
    assert_eq!(
        document
            .admit_native_target(&focus, button_node, UiPermission::Focus)
            .unwrap(),
        button
    );
    let heading = document.node_for_key(UiKey::new(2, 1)).unwrap();
    assert!(document
        .admit_native_target(&focus, heading, UiPermission::Focus)
        .is_err());
    assert!(document
        .admit_native_event(&context, heading, UiEventKind::Click)
        .is_err());

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
                "element": { "kind": "text-field", "content": "Alicia" },
                "semantics": {
                    "role": "text-field",
                    "label": "Display name",
                    "focusable": true
                },
                "style": {
                    "box": { "height": { "unit": "px", "value": 36 } },
                    "paint": { "background": "#FFFFFFFF" }
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
    let mut replay = document.scene().clone();
    let projection = document.apply_frame_with_patches(&frame).unwrap();
    replay
        .apply_batch_at(2, &projection.patches)
        .expect("projected successor batch");
    assert_eq!(document.revision(), 3);
    assert_eq!(document.scene().version(), 3);
    assert_eq!(&replay, document.scene());
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
        document.scene().find(field).unwrap().text_content(),
        Some("Alicia")
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
    assert!(document.committed_resource("logo").is_none());
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
    assert_eq!(
        document
            .committed_resource("logo")
            .map(|resource| resource.generation),
        Some(1)
    );
    document
        .apply_frame(&operate(
            document.revision(),
            serde_json::json!([{
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "prepare"
            }]),
        ))
        .unwrap();
    assert_eq!(
        document
            .committed_resource("logo")
            .map(|resource| resource.generation),
        Some(1)
    );
    assert_eq!(
        document
            .prepared_resource("logo")
            .map(|resource| resource.generation),
        Some(2)
    );
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
    assert!(document.committed_resource("logo").is_none());
    assert_eq!(
        document
            .prepared_resource("logo")
            .map(|resource| resource.generation),
        Some(2)
    );
    document
        .apply_frame(&operate(
            document.revision(),
            serde_json::json!([{
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "commit"
            }]),
        ))
        .unwrap();
    assert_eq!(
        document
            .committed_resource("logo")
            .map(|resource| resource.generation),
        Some(2)
    );
    document
        .apply_frame(&operate(
            document.revision(),
            serde_json::json!([{
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
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
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "prepare"
            }])
        ))
        .is_err());
    assert_eq!(document.revision(), revision);
}

#[test]
fn image_styles_require_committed_resources_and_retire_with_their_nodes() {
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let image = serde_json::json!({
        "key": { "semantic_id": 9, "generation": 1 },
        "element": { "kind": "image" },
        "semantics": { "role": "image", "label": "Logo" },
        "style": {
            "box": {
                "width": { "unit": "px", "value": 64 },
                "height": { "unit": "px", "value": 32 }
            },
            "image": {
                "resource": { "id": "logo", "generation": 1, "kind": "image" }
            }
        }
    });
    let image_frame = |revision: u64, ops: serde_json::Value| {
        let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        frame["expected_revision"] = serde_json::json!(revision);
        frame["next_revision"] = serde_json::json!(revision + 1);
        frame["ops"] = ops;
        UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap()
    };

    let frame = image_frame(
        2,
        serde_json::json!([
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 1, "kind": "image" },
                "admission": "prepare"
            },
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 1, "kind": "image" },
                "admission": "commit"
            },
            {
                "op": "insert",
                "parent": { "semantic_id": 1, "generation": 1 },
                "index": 3,
                "node": image.clone()
            }
        ]),
    );
    document.apply_frame(&frame).unwrap();
    let active = document.active_resource("logo").unwrap();
    assert_eq!((active.generation, active.kind), (1, ResourceKind::Image));
    let image_node = document.node_for_key(UiKey::new(9, 1)).unwrap();
    assert_eq!(
        document
            .scene()
            .find(image_node)
            .unwrap()
            .style
            .image
            .as_ref()
            .unwrap()
            .source,
        "logo"
    );

    let mismatch = image_frame(
        document.revision(),
        serde_json::json!([
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "prepare"
            },
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "commit"
            }
        ]),
    );
    assert!(document.apply_frame(&mismatch).is_err());
    assert_eq!(document.active_resource("logo").unwrap().generation, 1);

    let upgrade = image_frame(
        document.revision(),
        serde_json::json!([
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "prepare"
            },
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "commit"
            },
            {
                "op": "update",
                "key": { "semantic_id": 9, "generation": 1 },
                "fields": {
                    "style": {
                        "box": {
                            "width": { "unit": "px", "value": 64 },
                            "height": { "unit": "px", "value": 32 }
                        },
                        "image": {
                            "resource": { "id": "logo", "generation": 2, "kind": "image" }
                        }
                    }
                }
            }
        ]),
    );
    document.apply_frame(&upgrade).unwrap();
    assert_eq!(document.active_resource("logo").unwrap().generation, 2);
    assert_eq!(document.resource_ref("logo").unwrap().generation, 2);

    let revision = document.revision();
    let release = image_frame(
        revision,
        serde_json::json!([{
            "op": "resource",
            "resource": { "id": "logo", "generation": 2, "kind": "image" },
            "admission": "release"
        }]),
    );
    assert!(document.apply_frame(&release).is_err());
    assert_eq!(document.revision(), revision);
    assert!(document.active_resource("logo").is_some());

    let remove_and_release = image_frame(
        revision,
        serde_json::json!([
            { "op": "remove", "key": { "semantic_id": 9, "generation": 1 } },
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 2, "kind": "image" },
                "admission": "release"
            }
        ]),
    );
    document.apply_frame(&remove_and_release).unwrap();
    assert!(document.active_resource("logo").is_none());
    assert!(document.node_for_key(UiKey::new(9, 1)).is_none());

    let prepared_only = image_frame(
        2,
        serde_json::json!([
            {
                "op": "resource",
                "resource": { "id": "logo", "generation": 1, "kind": "image" },
                "admission": "prepare"
            },
            {
                "op": "insert",
                "parent": { "semantic_id": 1, "generation": 1 },
                "index": 3,
                "node": image
            }
        ]),
    );
    let mut rejected = NativeUiDocument::from_json_slice(&bytes).unwrap();
    assert!(rejected.apply_frame(&prepared_only).is_err());
    assert_eq!(rejected.revision(), 2);
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

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn projected_successor_patches_update_an_active_scene_host_view() {
    use jasper_ui_engine::native::host::SceneHostView;
    use jasper_ui_engine::native::{EventReply, PointerAction, SceneEventKind};

    const OWNER: u64 = 72;
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut view = SceneHostView::new(OWNER, document.scene(), &font, 480, 320).unwrap();
    let button = document.node_for_key(UiKey::new(4, 1)).unwrap();
    assert!(view.focus(OWNER, 2, button, &mut |_| {}));
    let before = view.node_bounds(OWNER, 2, button).unwrap();
    let mut activated = 0usize;
    assert!(view.pointer(
        OWNER,
        2,
        PointerAction::Down,
        before.x() + before.width() / 2.0,
        before.y() + before.height() / 2.0,
        true,
        &mut |event| {
            activated += usize::from(event.kind == SceneEventKind::Activate);
            EventReply::default()
        },
        &mut |_| {},
    ));

    let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    frame["expected_revision"] = serde_json::json!(2);
    frame["next_revision"] = serde_json::json!(3);
    frame["ops"] = serde_json::json!([
        {
            "op": "update",
            "key": { "semantic_id": 3, "generation": 1 },
            "fields": {
                "element": { "kind": "text-field", "content": "Alicia" },
                "semantics": {
                    "role": "text-field",
                    "label": "Display name",
                    "focusable": true
                },
                "style": {
                    "box": { "height": { "unit": "px", "value": 36 } },
                    "paint": { "background": "#FFFFFFFF" }
                }
            }
        },
        {
            "op": "move",
            "key": { "semantic_id": 4, "generation": 1 },
            "parent": { "semantic_id": 1, "generation": 1 },
            "index": 1
        }
    ]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
    let update = document.apply_frame_with_patches(&frame).unwrap();
    view.try_apply(
        OWNER,
        update.expected_revision,
        update.next_revision,
        &update.patches,
    )
    .expect("live host accepts the same projected frame");
    assert_eq!(
        view.focused_node(OWNER, update.next_revision),
        Some(button),
        "moving a focused node preserves its UiKey-backed Native identity"
    );
    let after = view
        .node_bounds(OWNER, update.next_revision, button)
        .unwrap();
    assert!(view.pointer(
        OWNER,
        update.next_revision,
        PointerAction::Up,
        after.x() + after.width() / 2.0,
        after.y() + after.height() / 2.0,
        false,
        &mut |event| {
            activated += usize::from(event.kind == SceneEventKind::Activate);
            EventReply::default()
        },
        &mut |_| {},
    ));
    assert_eq!(
        activated, 1,
        "pointer capture survives the identity-preserving move"
    );

    let semantics = view.semantics(OWNER, update.next_revision).unwrap();
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert_eq!(
        semantics
            .nodes
            .iter()
            .find(|item| item.node == field)
            .unwrap()
            .semantics
            .label,
        "Display name"
    );
    assert_eq!(
        view.semantics(OWNER, update.next_revision)
            .unwrap()
            .nodes
            .iter()
            .find(|item| item.semantics.role == Role::Form)
            .unwrap()
            .children[1],
        document.node_for_key(UiKey::new(4, 1)).unwrap()
    );
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn host_renderer_gates_typed_ui_frame_atomically_across_suspend_resume() {
    use jasper_ui_engine::native::host::{HostRenderer, TypedUiApplyError};
    use jasper_ui_engine::native::SceneUpdateError;

    const OWNER: u64 = 76;
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 320;
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new(OWNER, document.scene(), &font, WIDTH, HEIGHT).unwrap();
    let active = host.surface_status(OWNER).unwrap();
    let initial_scene = document.scene().clone();

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
                }
            }
        },
        {
            "op": "env",
            "env": { "reduce_motion": true }
        }
    ]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
    let suspended = host
        .suspend(OWNER, 2, active.generation, &mut |_| {})
        .unwrap();
    assert!(matches!(
        host.try_apply_ui_frame(OWNER, &mut document, &frame),
        Err(TypedUiApplyError::Host(SceneUpdateError::Closed))
    ));
    assert_eq!(document.revision(), 2);
    assert_eq!(document.scene(), &initial_scene);

    let resumed = host.resume(OWNER, suspended, WIDTH, HEIGHT, 1.0).unwrap();
    let update = host
        .try_apply_ui_frame(OWNER, &mut document, &frame)
        .unwrap();
    assert_eq!(update.next_revision, 3);
    assert_eq!(document.revision(), 3);
    assert_eq!(host.reduce_motion(OWNER), Some(true));
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    assert!(host.render_to_surface(
        OWNER,
        document.revision(),
        resumed,
        &mut pixels,
        (WIDTH * 4) as usize,
        64 * 1024 * 1024,
    ));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn host_renderer_applies_initial_typed_ir_environment() {
    use jasper_ui_engine::native::host::HostRenderer;

    const OWNER: u64 = 79;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "op": "env",
            "env": { "reduce_motion": true, "appearance": "dark" }
        }));
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    assert_eq!(host.reduce_motion(OWNER), Some(true));
    assert_eq!(
        document.environment().appearance,
        Some(jasper_ui_engine::ui_ir::Appearance::Dark)
    );
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn typed_focus_text_and_ime_use_the_same_admitted_ui_key() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{
        EventReply, HostKeyInput, KeyLocation, Modifiers, TypedUiTargetError,
    };

    const OWNER: u64 = 83;
    let document = NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    let context = UiEventContext {
        target: UiKey::new(3, 1),
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "text-host",
            [
                UiPermission::Focus,
                UiPermission::Keyboard,
                UiPermission::TextEdit,
                UiPermission::Ime,
            ],
        ),
    };
    let mut focus_events = 0usize;
    assert_eq!(
        host.typed_focus(
            OWNER,
            2,
            generation,
            &document,
            &context,
            field,
            &mut |_| focus_events += 1,
        )
        .unwrap(),
        UiKey::new(3, 1)
    );
    assert_eq!(focus_events, 1);
    assert_eq!(
        host.typed_text_input(OWNER, 2, generation, &document, &context, field, "!")
            .unwrap(),
        UiKey::new(3, 1)
    );
    assert_eq!(host.edit_state(OWNER, 2).unwrap().unwrap().text, "Alice!");
    assert_eq!(
        host.typed_key(
            OWNER,
            2,
            generation,
            &document,
            &context,
            field,
            &HostKeyInput {
                key: "Backspace",
                code: "Backspace",
                pressed: true,
                repeat: false,
                composing: false,
                modifiers: Modifiers::default(),
                location: KeyLocation::Standard,
            },
            &mut |_| EventReply::default(),
            &mut |_| {},
        )
        .unwrap(),
        UiKey::new(3, 1)
    );
    assert_eq!(host.edit_state(OWNER, 2).unwrap().unwrap().text, "Alice");
    assert_eq!(
        host.typed_set_preedit(
            OWNER,
            2,
            generation,
            &document,
            &context,
            field,
            Some("中"),
            None,
        )
        .unwrap(),
        UiKey::new(3, 1)
    );
    assert_eq!(
        host.edit_state(OWNER, 2)
            .unwrap()
            .unwrap()
            .preedit
            .as_deref(),
        Some("中")
    );

    let wrong_target = UiEventContext {
        target: UiKey::new(2, 1),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_focus(
            OWNER,
            2,
            generation,
            &document,
            &wrong_target,
            field,
            &mut |_| {},
        ),
        Err(TypedUiTargetError::Admission(_))
    ));
    let no_text = UiEventContext {
        permission_context: UiPermissionContext::new("focus-only", [UiPermission::Focus]),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_text_input(OWNER, 2, generation, &document, &no_text, field, "x"),
        Err(TypedUiTargetError::Admission(_))
    ));
    let no_keyboard = UiEventContext {
        permission_context: UiPermissionContext::new(
            "text-only",
            [UiPermission::Focus, UiPermission::TextEdit],
        ),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_key(
            OWNER,
            2,
            generation,
            &document,
            &no_keyboard,
            field,
            &HostKeyInput {
                key: "Backspace",
                code: "Backspace",
                pressed: true,
                repeat: false,
                composing: false,
                modifiers: Modifiers::default(),
                location: KeyLocation::Standard,
            },
            &mut |_| EventReply::default(),
            &mut |_| {},
        ),
        Err(TypedUiTargetError::Admission(_))
    ));
    assert!(matches!(
        host.typed_focus(
            OWNER,
            3,
            generation,
            &document,
            &context,
            field,
            &mut |_| {},
        ),
        Err(TypedUiTargetError::RevisionMismatch { .. })
    ));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn typed_keyboard_activation_requires_activate_binding() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{
        EventReply, HostKeyInput, KeyLocation, Modifiers, TypedUiTargetError,
    };

    const OWNER: u64 = 85;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"][0]["node"]["children"][2]["events"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "event": "activate",
            "action": "keyboard-save"
        }));
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let button = document.node_for_key(UiKey::new(4, 1)).unwrap();
    let context = UiEventContext {
        target: UiKey::new(4, 1),
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "keyboard-host",
            [
                UiPermission::Focus,
                UiPermission::Keyboard,
                UiPermission::Activate,
            ],
        ),
    };
    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &context,
        button,
        &mut |_| {},
    )
    .unwrap();
    let enter = HostKeyInput {
        key: "Enter",
        code: "Enter",
        pressed: true,
        repeat: false,
        composing: false,
        modifiers: Modifiers::default(),
        location: KeyLocation::Standard,
    };
    let mut activations = 0usize;
    let admitted = host
        .typed_key_with_activation(
            OWNER,
            2,
            generation,
            &document,
            &context,
            button,
            &enter,
            &mut |_| EventReply::default(),
            &mut |_| {},
            &mut |_| activations += 1,
        )
        .unwrap()
        .unwrap();
    assert_eq!(admitted.action, "keyboard-save");
    assert_eq!(activations, 1);

    let click_only = NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER + 1, &click_only, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER + 1).unwrap().generation;
    let button = click_only.node_for_key(UiKey::new(4, 1)).unwrap();
    host.typed_focus(
        OWNER + 1,
        2,
        generation,
        &click_only,
        &context,
        button,
        &mut |_| {},
    )
    .unwrap();
    assert!(matches!(
        host.typed_key_with_activation(
            OWNER + 1,
            2,
            generation,
            &click_only,
            &context,
            button,
            &enter,
            &mut |_| EventReply::default(),
            &mut |_| {},
            &mut |_| unreachable!("unbound keyboard activation must not reach Framework"),
        ),
        Err(TypedUiTargetError::Admission(_))
    ));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn typed_geometry_queries_use_ui_keys_and_live_generation() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::TypedUiGeometryError;

    const OWNER: u64 = 84;
    let document = NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let root = host
        .typed_node_bounds(OWNER, 2, generation, &document, UiKey::new(1, 1))
        .unwrap()
        .unwrap();
    assert!((root.width() - 480.0).abs() < 0.5);
    assert!(matches!(
        host.typed_node_bounds(OWNER, 2, generation, &document, UiKey::new(99, 1)),
        Err(TypedUiGeometryError::UnknownKey(_))
    ));
    assert!(matches!(
        host.typed_node_bounds(OWNER, 2, generation + 1, &document, UiKey::new(1, 1)),
        Err(TypedUiGeometryError::HostRejected)
    ));
    assert!(matches!(
        host.typed_node_bounds(OWNER, 3, generation, &document, UiKey::new(1, 1)),
        Err(TypedUiGeometryError::RevisionMismatch { .. })
    ));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn accessibility_tree_projects_losslessly_to_ui_keys() {
    use jasper_ui_engine::native::host::{HostRenderer, TypedUiAccessibilityError};

    const OWNER: u64 = 80;
    let document = NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new(OWNER, document.scene(), &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    assert!(host.focus(OWNER, document.revision(), generation, field, &mut |_| {}));
    let typed = host
        .typed_accessibility_tree(OWNER, document.revision(), generation, &document)
        .unwrap();
    assert_eq!(typed.root, Some(UiKey::new(1, 1)));
    assert_eq!(typed.focused, Some(UiKey::new(3, 1)));
    assert_eq!(
        typed.nodes.iter().map(|node| node.key).collect::<Vec<_>>(),
        vec![
            UiKey::new(1, 1),
            UiKey::new(2, 1),
            UiKey::new(3, 1),
            UiKey::new(4, 1),
        ]
    );
    assert_eq!(
        typed.nodes[0].children,
        vec![UiKey::new(2, 1), UiKey::new(3, 1), UiKey::new(4, 1),]
    );

    assert!(matches!(
        host.typed_accessibility_tree(OWNER, document.revision(), generation + 1, &document,),
        Err(TypedUiAccessibilityError::HostRejected)
    ));
    assert!(matches!(
        host.typed_accessibility_tree(OWNER, document.revision() + 1, generation, &document,),
        Err(TypedUiAccessibilityError::RevisionMismatch { .. })
    ));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn accessibility_update_maps_changed_and_removed_nodes_to_ui_keys() {
    use jasper_ui_engine::native::host::{HostRenderer, TypedUiAccessibilityError};

    const OWNER: u64 = 81;
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new(OWNER, document.scene(), &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let previous_native = host
        .accessibility_tree(OWNER, document.revision(), generation)
        .unwrap();
    let previous_document = document.clone();

    let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    frame["expected_revision"] = serde_json::json!(2);
    frame["next_revision"] = serde_json::json!(3);
    frame["ops"] = serde_json::json!([{
        "op": "remove",
        "key": { "semantic_id": 2, "generation": 1 }
    }]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
    host.try_apply_ui_frame(OWNER, &mut document, &frame)
        .unwrap();
    let next_native = host
        .accessibility_tree(OWNER, document.revision(), generation)
        .unwrap();
    let typed = host
        .typed_accessibility_update(
            OWNER,
            document.revision(),
            generation,
            &previous_native,
            &next_native,
            &previous_document,
            &document,
        )
        .unwrap();
    assert_eq!(typed.from_version, 2);
    assert_eq!(typed.scene_version, 3);
    assert_eq!(typed.root, Some(UiKey::new(1, 1)));
    assert_eq!(typed.removed, vec![UiKey::new(2, 1)]);
    assert!(typed.nodes.iter().all(|node| node.key != UiKey::new(2, 1)));
    assert!(typed
        .nodes
        .iter()
        .find(|node| node.key == UiKey::new(1, 1))
        .is_some_and(|node| { node.children == vec![UiKey::new(3, 1), UiKey::new(4, 1),] }));
    assert!(matches!(
        host.typed_accessibility_update(
            OWNER,
            document.revision(),
            generation + 1,
            &previous_native,
            &next_native,
            &previous_document,
            &document,
        ),
        Err(TypedUiAccessibilityError::HostRejected)
    ));
}

#[cfg(all(feature = "native-blitz-cpu-pending", feature = "accesskit-pending"))]
#[test]
fn typed_accesskit_actions_require_ui_key_and_permissions() {
    use jasper_ui_engine::native::host::accesskit::{
        Action, ActionRequest, NodeId as KitId, TreeId,
    };
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{TypedAccessibilityAction, TypedUiAccessibilityActionError};

    const OWNER: u64 = 82;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"][0]["node"]["children"][2]["events"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "event": "accessibility-action",
            "action": "save-accessible"
        }));
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let button = document.node_for_key(UiKey::new(4, 1)).unwrap();
    let request = |action| ActionRequest {
        action,
        target_node: KitId(button.0),
        target_tree: TreeId::ROOT,
        data: None,
    };
    let context = UiEventContext {
        target: UiKey::new(4, 1),
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "accessibility-host",
            [UiPermission::AccessibilityAction, UiPermission::Focus],
        ),
    };

    let mut activated = 0usize;
    let result = host
        .typed_accesskit_action(
            OWNER,
            2,
            generation,
            &document,
            &context,
            &request(Action::Click),
            &mut |_| {},
            &mut |_| activated += 1,
        )
        .unwrap();
    assert!(matches!(
        result,
        TypedAccessibilityAction::Activated(ref event) if event.action == "save-accessible"
    ));
    assert_eq!(activated, 1);

    let mut focused = 0usize;
    let result = host
        .typed_accesskit_action(
            OWNER,
            2,
            generation,
            &document,
            &context,
            &request(Action::Focus),
            &mut |_| focused += 1,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(result, TypedAccessibilityAction::Focused(UiKey::new(4, 1)));
    assert_eq!(focused, 1);

    let focus_only = UiEventContext {
        permission_context: UiPermissionContext::new("focus-only", [UiPermission::Focus]),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_accesskit_action(
            OWNER,
            2,
            generation,
            &document,
            &focus_only,
            &request(Action::Click),
            &mut |_| {},
            &mut |_| {},
        ),
        Err(TypedUiAccessibilityActionError::Admission(_))
    ));
    assert!(matches!(
        host.typed_accesskit_action(
            OWNER,
            2,
            generation,
            &document,
            &context,
            &request(Action::Blur),
            &mut |_| {},
            &mut |_| {},
        ),
        Err(TypedUiAccessibilityActionError::ActionNotSupported)
    ));
    assert_eq!(host.focused_node(OWNER, 2), Some(button));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn same_data_table_fixture_executes_and_exposes_table_semantics() {
    use jasper_ui_engine::native::host::SceneHostView;

    const OWNER: u64 = 71;
    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 320;
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let document = NativeUiDocument::from_json_slice(&fixture("data-table.ir.json")).unwrap();
    let mut view = SceneHostView::new(OWNER, document.scene(), &font, WIDTH, HEIGHT)
        .expect("Native table construction");
    let semantics = view.semantics(OWNER, document.revision()).unwrap();
    let role = |key| {
        semantics
            .nodes
            .iter()
            .find(|item| item.node == document.node_for_key(UiKey::new(key, 1)).unwrap())
            .map(|item| item.semantics.role)
            .unwrap()
    };
    assert_eq!(role(20), Role::Table);
    assert_eq!(role(21), Role::TableRow);
    assert_eq!(role(22), Role::TableCell);

    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    assert!(view.render_to_surface(
        OWNER,
        document.revision(),
        &mut pixels,
        (WIDTH * 4) as usize,
        48 * 1024 * 1024,
    ));
    assert!(pixels
        .chunks_exact(4)
        .any(|pixel| pixel.iter().any(|byte| *byte != 0)));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn same_dialog_fixture_executes_with_candidate_overlay_metadata() {
    use jasper_ui_engine::native::host::SceneHostView;

    const OWNER: u64 = 73;
    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 480;
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let document = NativeUiDocument::from_json_slice(&fixture("dialog-overlay.ir.json")).unwrap();
    let mut view = SceneHostView::new(OWNER, document.scene(), &font, WIDTH, HEIGHT)
        .expect("Native dialog construction");
    let overlays = view.overlays(OWNER, document.revision()).unwrap();
    assert_eq!(overlays.len(), 1);
    let dialog = document.node_for_key(UiKey::new(32, 1)).unwrap();
    assert_eq!(overlays[0].overlay, dialog);
    assert_eq!(overlays[0].spec.layer, 100);
    assert!(overlays[0].spec.modal);
    assert_eq!(overlays[0].spec.scrim.map(|color| color.a), Some(0x80));
    assert!(overlays[0].placement.is_some());

    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    assert!(view.render_to_surface(
        OWNER,
        document.revision(),
        &mut pixels,
        (WIDTH * 4) as usize,
        64 * 1024 * 1024,
    ));
    assert!(pixels
        .chunks_exact(4)
        .any(|pixel| pixel.iter().any(|byte| *byte != 0)));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn grid_candidate_enters_real_native_layout() {
    use jasper_ui_engine::native::host::SceneHostView;

    const OWNER: u64 = 78;
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut view = SceneHostView::new(OWNER, document.scene(), &font, 480, 320).unwrap();
    let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    frame["expected_revision"] = serde_json::json!(2);
    frame["next_revision"] = serde_json::json!(3);
    frame["ops"] = serde_json::json!([{
        "op": "insert",
        "parent": { "semantic_id": 1, "generation": 1 },
        "index": 3,
        "node": {
            "key": { "semantic_id": 10, "generation": 1 },
            "element": { "kind": "container" },
            "semantics": { "role": "container", "label": "Grid" },
            "style": {
                "box": { "height": { "unit": "px", "value": 40 } },
                "grid": {
                    "columns": [
                        { "unit": "px", "value": 100 },
                        { "unit": "fr", "value": 1 }
                    ],
                    "rows": [{ "unit": "px", "value": 40 }],
                    "gap": 4
                }
            },
            "children": [
                {
                    "key": { "semantic_id": 11, "generation": 1 },
                    "element": { "kind": "text", "content": "A" },
                    "semantics": { "role": "text", "label": "A" }
                },
                {
                    "key": { "semantic_id": 12, "generation": 1 },
                    "element": { "kind": "text", "content": "B" },
                    "semantics": { "role": "text", "label": "B" }
                }
            ]
        }
    }]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
    let update = document.apply_frame_with_patches(&frame).unwrap();
    view.try_apply(
        OWNER,
        update.expected_revision,
        update.next_revision,
        &update.patches,
    )
    .unwrap();
    let columns = document.mapped_properties()["grid.columns"];
    assert_eq!(columns.support, SupportLevel::Candidate);
    assert!(!columns.host_accepted);
    let a = document.node_for_key(UiKey::new(11, 1)).unwrap();
    let b = document.node_for_key(UiKey::new(12, 1)).unwrap();
    let a = view.node_bounds(OWNER, update.next_revision, a).unwrap();
    let b = view.node_bounds(OWNER, update.next_revision, b).unwrap();
    assert!((a.width() - 100.0).abs() < 0.5, "first grid track: {a:?}");
    assert!(b.x() >= a.x() + a.width() + 3.5, "grid gap: {a:?} {b:?}");
    assert!(b.width() > 300.0, "fractional second track: {b:?}");
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn committed_ir_resource_reaches_the_live_native_image_store() {
    use jasper_ui_engine::native::host::{HostRenderer, TypedUiResourceError};

    const OWNER: u64 = 75;
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 480;
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new(OWNER, document.scene(), &font, WIDTH, HEIGHT).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    frame["expected_revision"] = serde_json::json!(2);
    frame["next_revision"] = serde_json::json!(3);
    frame["ops"] = serde_json::json!([
        {
            "op": "resource",
            "resource": { "id": "logo", "generation": 1, "kind": "image" },
            "admission": "prepare"
        },
        {
            "op": "resource",
            "resource": { "id": "logo", "generation": 1, "kind": "image" },
            "admission": "commit"
        },
        {
            "op": "insert",
            "parent": { "semantic_id": 1, "generation": 1 },
            "index": 3,
            "node": {
                "key": { "semantic_id": 9, "generation": 1 },
                "element": { "kind": "image" },
                "semantics": { "role": "image", "label": "Logo" },
                "style": {
                    "box": {
                        "width": { "unit": "px", "value": 64 },
                        "height": { "unit": "px", "value": 32 }
                    },
                    "image": {
                        "resource": { "id": "logo", "generation": 1, "kind": "image" }
                    }
                }
            }
        }
    ]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap();
    let update = document.apply_frame_with_patches(&frame).unwrap();
    host.try_apply_ui_ir(OWNER, &update).unwrap();
    assert_eq!(
        document
            .committed_resource("logo")
            .map(|resource| resource.generation),
        Some(1)
    );

    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 140, 220, 255]));
    let mut encoded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let resource = document.committed_resource("logo").unwrap().clone();
    let mut stale = resource.clone();
    stale.generation += 1;
    assert!(matches!(
        host.insert_ui_image(OWNER, &document, &stale, encoded.get_ref(), 1.0),
        Err(TypedUiResourceError::ResourceNotCommitted { .. })
    ));
    host.insert_ui_image(OWNER, &document, &resource, encoded.get_ref(), 1.0)
        .unwrap();
    let image_node = document.node_for_key(UiKey::new(9, 1)).unwrap();
    assert!(host
        .node_bounds(OWNER, update.next_revision, generation, image_node)
        .is_some());
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    assert!(host.render_to_surface(
        OWNER,
        update.next_revision,
        generation,
        &mut pixels,
        (WIDTH * 4) as usize,
        64 * 1024 * 1024,
    ));
    let report = host.image_report(OWNER).unwrap();
    assert_eq!(report.resources, 1);
    assert!(report.failed.is_empty(), "{:?}", report.failed);

    assert!(matches!(
        host.remove_ui_image(OWNER, &document, "logo"),
        Err(TypedUiResourceError::ResourceStillActive { .. })
    ));
    let mut release: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    release["expected_revision"] = serde_json::json!(document.revision());
    release["next_revision"] = serde_json::json!(document.revision() + 1);
    release["ops"] = serde_json::json!([
        { "op": "remove", "key": { "semantic_id": 9, "generation": 1 } },
        {
            "op": "resource",
            "resource": { "id": "logo", "generation": 1, "kind": "image" },
            "admission": "release"
        }
    ]);
    let release = UiFrame::from_json_slice(&serde_json::to_vec(&release).unwrap()).unwrap();
    host.try_apply_ui_frame(OWNER, &mut document, &release)
        .unwrap();
    assert_eq!(
        host.remove_ui_image(OWNER, &document, "logo").unwrap(),
        Some(true)
    );
    assert_eq!(host.image_report(OWNER).unwrap().resources, 0);
}

#[cfg(feature = "gpu-pending")]
#[test]
fn same_settings_fixture_executes_on_native_gpu_and_tracks_cpu() {
    use jasper_ui_engine::engine::Engine;
    use jasper_ui_engine::native::host::GpuRenderer;

    const OWNER: u64 = 74;
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 320;
    const BUDGET: usize = 64 * 1024 * 1024;
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let document = NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
    let mut engine =
        Engine::new(OWNER, document.scene(), &font, WIDTH, HEIGHT).expect("Native scene");
    let mut gpu = GpuRenderer::new_headless().expect("Native GPU adapter");
    eprintln!("typed UI IR GPU adapter: {:?}", gpu.adapter_info());

    let mut cpu = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    assert!(engine.render_to_surface(
        OWNER,
        document.revision(),
        &mut cpu,
        (WIDTH * 4) as usize,
        BUDGET,
    ));
    assert!(engine.invalidate_frame(OWNER));
    let gpu = engine
        .render_gpu(OWNER, document.revision(), &mut gpu, BUDGET)
        .expect("GPU frame");
    assert_eq!(gpu.len(), cpu.len());
    assert!(gpu
        .chunks_exact(4)
        .any(|pixel| pixel.iter().any(|byte| *byte != 0)));

    let mut differing = 0usize;
    let mut max_delta = 0u8;
    for (gpu, cpu) in gpu.chunks_exact(4).zip(cpu.chunks_exact(4)) {
        let delta = gpu
            .iter()
            .zip(cpu)
            .map(|(gpu, cpu)| gpu.abs_diff(*cpu))
            .max()
            .unwrap_or(0);
        differing += usize::from(delta != 0);
        max_delta = max_delta.max(delta);
    }
    eprintln!(
        "typed UI IR gpu vs cpu: {differing}/{} pixels differ, max channel delta {max_delta}",
        WIDTH * HEIGHT
    );
    assert!(max_delta <= 1, "channel delta {max_delta} exceeds 1");
    assert!(
        differing < (WIDTH * HEIGHT / 20) as usize,
        "more than 5% of pixels differ"
    );
}
