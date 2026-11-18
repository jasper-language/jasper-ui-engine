use jasper_ui_engine::semantics::Role;
use jasper_ui_engine::ui_ir::{
    MappingIssueKind, NativeActivationSource, NativeUiDocument, NativeUiFrameError, ResourceKind,
    SupportLevel, UiEventContext, UiEventKind, UiFrame, UiKey, UiPermission, UiPermissionContext,
    NATIVE_PROPERTY_PROFILE, native_property_profile,
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
fn typed_property_profile_matches_the_pinned_registry() {
    let bytes = fixture("typed-properties-v0.json");
    assert_eq!(fnv1a(&bytes), 0xd982_c4a7_aecc_04be);
    let registry: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let properties = registry["properties"].as_object().unwrap();
    assert_eq!(properties.len(), NATIVE_PROPERTY_PROFILE.len());

    let support = |name: &str| match name {
        "required" => SupportLevel::Required,
        "optional" => SupportLevel::Optional,
        "host-owned" => SupportLevel::HostOwned,
        "web-only" => SupportLevel::WebOnly,
        "native-only" => SupportLevel::NativeOnly,
        "candidate" => SupportLevel::Candidate,
        "unsupported" => SupportLevel::Unsupported,
        other => panic!("unknown registry support level {other:?}"),
    };
    let mut seen = std::collections::BTreeSet::new();
    for profile in NATIVE_PROPERTY_PROFILE {
        assert!(
            seen.insert(profile.property),
            "duplicate Native profile property {:?}",
            profile.property
        );
        let property = properties
            .get(profile.property)
            .unwrap_or_else(|| panic!("{} is absent from the registry", profile.property));
        assert_eq!(
            profile.support,
            support(property["support"].as_str().unwrap()),
            "{} support differs from the registry",
            profile.property
        );
        if let Some(host_acceptance) = property.get("host_acceptance") {
            assert_eq!(
                profile.host_accepted,
                host_acceptance.as_bool().unwrap(),
                "{} host acceptance differs from the registry",
                profile.property
            );
        }
        if profile.support == SupportLevel::Candidate {
            assert!(
                !profile.host_accepted,
                "candidate property {} cannot claim host acceptance",
                profile.property
            );
        }
        assert_eq!(
            native_property_profile(profile.property),
            Some(profile)
        );
    }
    assert!(
        properties.keys().all(|name| seen.contains(name.as_str())),
        "Native profile omits a registered property"
    );

    for name in [
        "settings-page.ir.json",
        "data-table.ir.json",
        "dialog-overlay.ir.json",
    ] {
        let document = NativeUiDocument::from_json_slice(&fixture(name)).unwrap();
        for (property, mapped) in document.mapped_properties() {
            let profile = native_property_profile(property).unwrap();
            assert_eq!(mapped.support, profile.support, "{name}: {property}");
            assert_eq!(
                mapped.host_accepted, profile.host_accepted,
                "{name}: {property}"
            );
        }
    }
}

#[test]
fn native_style_projection_round_trips_representable_fields() {
    use jasper_ui_engine::style::{
        Align, Color, Display, FlexDirection, FlexWrap, FontStyle, Justify,
        Length as NativeLength, LineHeight, Position, TextAlign, TextAlignVertical,
        TextDirection, Style,
    };
    use jasper_ui_engine::ui_ir::{
        project_native_style, ElementKind, SemanticRole, UiElement, UiNode, UiOp, UiSemantics,
    };

    let source = Style {
        display: Display::Flex,
        direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        width: NativeLength::Px(120.0),
        height: NativeLength::Percent(50.0),
        min_width: NativeLength::Px(40.0),
        max_height: NativeLength::Px(240.0),
        padding: jasper_ui_engine::geometry::Edges::new(1.0, 2.0, 3.0, 4.0),
        margin: jasper_ui_engine::geometry::Edges::all(5.0),
        gap: 8.0,
        align_items: Align::Center,
        justify_content: Justify::SpaceBetween,
        flex_grow: 1.5,
        flex_shrink: 0.5,
        position: Position::Absolute,
        inset: jasper_ui_engine::style::Inset::new(
            NativeLength::Px(1.0),
            NativeLength::Px(2.0),
            NativeLength::Px(3.0),
            NativeLength::Px(4.0),
        ),
        z_index: Some(7),
        order: -2,
        background: Color::new(10, 20, 30, 255),
        color: Color::new(240, 230, 220, 255),
        border_width: 2.0,
        border_color: Color::new(100, 110, 120, 200),
        corner_radius: jasper_ui_engine::style::CornerRadii::new(3.0, 4.0, 5.0, 6.0),
        opacity: 0.75,
        font_family: Some("Host Sans".into()),
        font_size: 17.0,
        font_weight: 650,
        font_style: FontStyle::Italic,
        line_height: Some(LineHeight::Px(23.0)),
        text_align: TextAlign::Center,
        text_direction: TextDirection::Rtl,
        text_align_vertical: Some(TextAlignVertical::Center),
        ..Style::default()
    };

    let projection = project_native_style(&source, ElementKind::Container).unwrap();
    assert!(projection.is_lossless(), "{:?}", projection.losses);
    let frame = UiFrame {
        ir_version: jasper_ui_engine::ui_ir::IR_VERSION.into(),
        identity_domain: "compat/window".into(),
        owner: "host".into(),
        session: "session".into(),
        expected_revision: 1,
        next_revision: 2,
        ops: vec![UiOp::insert(
            None,
            0,
            UiNode {
                key: UiKey::new(1, 1),
                element: UiElement {
                    kind: ElementKind::Container,
                    content: None,
                    attributes: None,
                },
                semantics: UiSemantics {
                    role: SemanticRole::Container,
                    label: String::new(),
                    description: String::new(),
                    value: String::new(),
                    disabled: false,
                    focusable: false,
                    hidden: false,
                    selected: false,
                    level: None,
                },
                style: Some(projection.style),
                events: None,
                children: None,
            },
        )],
    };
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    assert_eq!(document.scene().root.style, source);
}

#[test]
fn native_style_projection_preserves_grid_and_reports_extended_field_losses() {
    use jasper_ui_engine::style::{
        Display, GridLine, GridTrack, ImageStyle, Length as NativeLength, LineHeight, Style,
        Transform2D,
    };
    use jasper_ui_engine::ui_ir::{
        project_native_style, project_native_style_with_context, ElementKind, NativeStyleContext,
        ResourceKind, ResourceRef, Track, TrackUnit, NATIVE_PROPERTY_PROFILE,
    };

    let grid = Style {
        display: Display::Grid,
        gap: 12.0,
        grid_columns: Some(vec![GridTrack::Px(40.0), GridTrack::Fr(1.0)]),
        grid_rows: Some(vec![GridTrack::Auto, GridTrack::Percent(25.0)]),
        grid_column: GridLine::Span(2),
        grid_row: GridLine::Line(3),
        ..Style::default()
    };
    let projection = project_native_style(&grid, ElementKind::Container).unwrap();
    assert!(projection.is_lossless(), "{:?}", projection.losses);
    let grid_style = projection.style.grid.unwrap();
    assert_eq!(grid_style.columns.unwrap()[1], Track::Value(
        jasper_ui_engine::ui_ir::TrackValue {
            unit: TrackUnit::Fr,
            value: 1.0,
        }
    ));
    assert_eq!(
        grid_style.column.unwrap(),
        jasper_ui_engine::ui_ir::GridPlacement {
            start: jasper_ui_engine::ui_ir::GridStart::Auto(
                jasper_ui_engine::ui_ir::GridAuto::Auto
            ),
            span: 2,
        }
    );
    let grid_profile = NATIVE_PROPERTY_PROFILE
        .iter()
        .find(|property| property.property == "grid.columns")
        .unwrap();
    assert_eq!(grid_profile.support, SupportLevel::Candidate);
    assert!(!grid_profile.host_accepted);

    let mut extended = Style {
        hit_slop: jasper_ui_engine::geometry::Edges::all(3.0),
        transform: Transform2D::translate(2.0, 3.0),
        line_height: Some(LineHeight::Multiple(1.5)),
        image: Some(ImageStyle::new("icon")),
        width: NativeLength::Px(10.0),
        ..Style::default()
    };
    extended.text_decoration.underline = true;
    let projection = project_native_style(&extended, ElementKind::Container).unwrap();
    assert!(!projection.is_lossless());
    let paths: Vec<_> = projection
        .losses
        .iter()
        .map(|loss| loss.path)
        .collect();
    for expected in [
        "style.hit_slop",
        "style.text_decoration",
        "style.transform",
        "style.line_height.multiple",
        "style.image",
    ] {
        assert!(paths.contains(&expected), "missing loss {expected}: {paths:?}");
    }

    let image_resource = ResourceRef {
        id: "icon".into(),
        generation: 4,
        kind: ResourceKind::Icon,
    };
    let image_projection = project_native_style_with_context(
        &extended,
        ElementKind::Container,
        NativeStyleContext {
            image_resource: Some(&image_resource),
            overlay_anchor: None,
        },
    )
    .unwrap();
    assert_eq!(
        image_projection.style.image,
        Some(jasper_ui_engine::ui_ir::ImageStyle {
            resource: image_resource,
            fit: Some(jasper_ui_engine::ui_ir::ImageFit::Fill),
            align_x: Some(0.5),
            align_y: Some(0.5),
            tint: None,
        })
    );
    assert!(!image_projection
        .losses
        .iter()
        .any(|loss| loss.path == "style.image"));

    let dialog = NativeUiDocument::from_json_slice(&fixture("dialog-overlay.ir.json")).unwrap();
    let overlay = dialog
        .project_compatibility_style(UiKey::new(32, 1))
        .unwrap()
        .unwrap();
    assert!(overlay.style.overlay.is_some());
    assert!(!overlay
        .losses
        .iter()
        .any(|loss| loss.path == "style.overlay"));
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
fn native_document_never_reuses_a_removed_ui_key_and_accepts_the_next_generation() {
    let bytes = fixture("settings-page.ir.json");
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let removed = UiKey::new(2, 1);
    assert_eq!(document.current_identity(2), Some(removed));

    let mut remove: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    remove["expected_revision"] = serde_json::json!(2);
    remove["next_revision"] = serde_json::json!(3);
    remove["ops"] = serde_json::json!([{ "op": "remove", "key": removed }]);
    let remove = UiFrame::from_json_slice(&serde_json::to_vec(&remove).unwrap()).unwrap();
    document.apply_frame(&remove).unwrap();
    assert!(document.node_for_key(removed).is_none());
    assert_eq!(document.current_identity(2), None);

    let reinsert = |generation: u64, revision: u64| {
        let mut frame: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        frame["expected_revision"] = serde_json::json!(revision);
        frame["next_revision"] = serde_json::json!(revision + 1);
        frame["ops"] = serde_json::json!([{
            "op": "insert",
            "parent": { "semantic_id": 1, "generation": 1 },
            "index": 0,
            "node": {
                "key": { "semantic_id": 2, "generation": generation },
                "element": { "kind": "text", "content": "Recreated" },
                "semantics": { "role": "heading", "label": "Recreated", "level": 1 }
            }
        }]);
        UiFrame::from_json_slice(&serde_json::to_vec(&frame).unwrap()).unwrap()
    };

    let reuse = reinsert(1, 3);
    assert!(matches!(
        document.apply_frame(&reuse),
        Err(NativeUiFrameError::Identity {
            error: jasper_ui_engine::ui_ir::IdentityError::UnexpectedGeneration { .. },
            ..
        })
    ));
    assert_eq!(document.revision(), 3);
    assert_eq!(document.current_identity(2), None);

    let next = UiKey::new(2, 2);
    document.apply_frame(&reinsert(2, 3)).unwrap();
    assert_eq!(document.revision(), 4);
    assert_eq!(document.current_identity(2), Some(next));
    assert!(document.node_for_key(next).is_some());
    assert!(document.node_for_key(removed).is_none());
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
    let initial_host_identity = host.snapshot_identity(OWNER);

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
    assert!(matches!(
        host.try_apply_ui_frame_at(
            OWNER,
            active.generation + 1,
            &mut document,
            &frame,
        ),
        Err(TypedUiApplyError::SurfaceRejected)
    ));
    assert_eq!(document.revision(), 2);
    assert_eq!(document.scene(), &initial_scene);
    assert_eq!(host.snapshot_identity(OWNER), initial_host_identity);
    let suspended = host
        .suspend(OWNER, 2, active.generation, &mut |_| {})
        .unwrap();
    assert!(matches!(
        host.try_apply_ui_frame(OWNER, &mut document, &frame),
        Err(TypedUiApplyError::SurfaceRejected)
    ));
    assert_eq!(document.revision(), 2);
    assert_eq!(document.scene(), &initial_scene);

    let resumed = host.resume(OWNER, suspended, WIDTH, HEIGHT, 1.0).unwrap();
    let update = host
        .try_apply_ui_frame_at(OWNER, resumed, &mut document, &frame)
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
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"][0]["node"]["children"][1]["events"] = serde_json::json!([
        { "event": "text-commit", "action": "name" },
        { "event": "focus", "action": "focus-name" },
        { "event": "key", "action": "key-name" },
        { "event": "ime-commit", "action": "ime-name" }
    ]);
    let document =
        NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
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
    let mut focus_events = Vec::new();
    assert_eq!(
        host.typed_focus(
            OWNER,
            2,
            generation,
            &document,
            &context,
            field,
            &mut |event| focus_events.push(event),
        )
        .unwrap(),
        UiKey::new(3, 1)
    );
    assert_eq!(
        focus_events,
        vec![jasper_ui_engine::native::TypedUiFocusEvent {
            target: UiKey::new(3, 1),
            focused: true,
            admitted: Some(jasper_ui_engine::ui_ir::AdmittedEvent {
                target: UiKey::new(3, 1),
                event: UiEventKind::Focus,
                action: "focus-name".into(),
            }),
        }]
    );
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        Some(UiKey::new(3, 1))
    );
    let no_permissions = UiEventContext {
        permission_context: UiPermissionContext::new("none", []),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_text_input(
            OWNER,
            2,
            generation + 1,
            &document,
            &no_permissions,
            field,
            "x",
        ),
        Err(TypedUiTargetError::HostRejected)
    ));
    assert_eq!(host.edit_state(OWNER, 2).unwrap().unwrap().text, "Alice");
    assert_eq!(
        host.typed_text_input(OWNER, 2, generation, &document, &context, field, "!")
            .unwrap(),
        UiKey::new(3, 1)
    );
    assert_eq!(host.edit_state(OWNER, 2).unwrap().unwrap().text, "Alice!");
    let edit = host.typed_edit_state(OWNER, 2, &document).unwrap().unwrap();
    assert_eq!(edit.target, UiKey::new(3, 1));
    assert_eq!(edit.text, "Alice!");
    let no_text_edit = UiEventContext {
        permission_context: UiPermissionContext::new(
            "keyboard-only",
            [UiPermission::Focus, UiPermission::Keyboard],
        ),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_key(
            OWNER,
            2,
            generation,
            &document,
            &no_text_edit,
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
        Err(TypedUiTargetError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::CapabilityDenied { .. }
        ))
    ));
    assert_eq!(host.edit_state(OWNER, 2).unwrap().unwrap().text, "Alice!");
    let mut typed_keys = Vec::new();
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
            &mut |event| {
                typed_keys.push(event);
                EventReply::default()
            },
            &mut |_| {},
        )
        .unwrap(),
        UiKey::new(3, 1)
    );
    assert_eq!(typed_keys.len(), 1);
    assert_eq!(typed_keys[0].target, UiKey::new(3, 1));
    assert_eq!(typed_keys[0].key, "Backspace");
    assert_eq!(
        typed_keys[0]
            .admitted
            .as_ref()
            .map(|event| (event.event, event.action.as_str())),
        Some((UiEventKind::Key, "key-name"))
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
    assert_eq!(
        host.typed_edit_state(OWNER, 2, &document)
            .unwrap()
            .unwrap()
            .preedit
            .as_deref(),
        Some("中")
    );
    let ime_only = UiEventContext {
        permission_context: UiPermissionContext::new("ime-only", [UiPermission::Ime]),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_ime_commit(
            OWNER,
            2,
            generation,
            &document,
            &ime_only,
            field,
            "中",
        ),
        Err(TypedUiTargetError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::CapabilityDenied { .. }
        ))
    ));
    assert_eq!(host.edit_state(OWNER, 2).unwrap().unwrap().text, "Alice");
    assert_eq!(
        host.typed_ime_commit(OWNER, 2, generation, &document, &context, field, "中")
            .unwrap()
            .map(|event| (event.event, event.action)),
        Some((UiEventKind::ImeCommit, "ime-name".into()))
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
    let button = document.node_for_key(UiKey::new(4, 1)).unwrap();
    let button_context = UiEventContext {
        target: UiKey::new(4, 1),
        permission_context: UiPermissionContext::new(
            "button-focus-with-blur-commit",
            [UiPermission::Focus, UiPermission::TextEdit],
        ),
        ..context.clone()
    };
    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &button_context,
        button,
        &mut |_| {},
    )
    .unwrap();
    assert!(matches!(
        host.typed_text_input(OWNER, 2, generation, &document, &context, field, "x"),
        Err(TypedUiTargetError::HostRejected)
    ));
    assert!(matches!(
        host.typed_set_preedit(
            OWNER,
            2,
            generation,
            &document,
            &context,
            field,
            Some("x"),
            None,
        ),
        Err(TypedUiTargetError::HostRejected)
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
fn typed_edit_drain_preserves_order_and_admits_commit_binding() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{
        EventReply, HostKeyInput, KeyLocation, Modifiers, SceneEditEventKind, TypedUiEditError,
    };

    const OWNER: u64 = 84;
    let mut document =
        NativeUiDocument::from_json_slice(&fixture("settings-page.ir.json")).unwrap();
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
            "edit-host",
            [
                UiPermission::Focus,
                UiPermission::Keyboard,
                UiPermission::TextEdit,
                UiPermission::Ime,
            ],
        ),
    };
    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &context,
        field,
        &mut |_| {},
    )
    .unwrap();
    host.typed_text_input(OWNER, 2, generation, &document, &context, field, "!")
        .unwrap();
    let mut successor: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    successor["expected_revision"] = serde_json::json!(2);
    successor["next_revision"] = serde_json::json!(3);
    successor["ops"] = serde_json::json!([{
        "op": "env",
        "env": { "reduce_motion": true }
    }]);
    let successor =
        UiFrame::from_json_slice(&serde_json::to_vec(&successor).unwrap()).unwrap();
    host.try_apply_ui_frame(OWNER, &mut document, &successor)
        .unwrap();
    let context = UiEventContext {
        frame_revision: 3,
        ..context
    };
    host.typed_key(
        OWNER,
        3,
        generation,
        &document,
        &context,
        field,
        &HostKeyInput {
            key: "Enter",
            code: "Enter",
            pressed: true,
            repeat: false,
            composing: false,
            modifiers: Modifiers::default(),
            location: KeyLocation::Standard,
        },
        &mut |_| EventReply::default(),
        &mut |_| {},
    )
    .unwrap();

    let no_text = UiEventContext {
        permission_context: UiPermissionContext::new(
            "keyboard-only",
            [
                UiPermission::Focus,
                UiPermission::Keyboard,
                UiPermission::Ime,
            ],
        ),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_take_edit_events(OWNER, &document, &no_text, false),
        Err(TypedUiEditError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));

    let events = host
        .typed_take_edit_events(OWNER, &document, &context, false)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].target, UiKey::new(3, 1));
    assert_eq!(
        events[0].kind,
        SceneEditEventKind::Changed("Alice!".into())
    );
    assert!(events[0].admitted.is_none());
    assert_eq!(events[1].target, UiKey::new(3, 1));
    assert_eq!(
        events[1].kind,
        SceneEditEventKind::Commit("Alice!".into())
    );
    assert_eq!(
        events[1]
            .admitted
            .as_ref()
            .map(|event| event.action.as_str()),
        Some("name")
    );
    assert!(matches!(
        host.typed_take_edit_events(OWNER + 1, &document, &context, false),
        Err(TypedUiEditError::HostRejected)
    ));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn typed_focus_change_preadmits_commit_and_ime_cancel() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{
        EventReply, PointerAction, SceneEditEventKind, TypedUiEditError, TypedUiPointerError,
        TypedUiTargetError,
    };

    const OWNER: u64 = 92;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"][0]["node"]["children"][1]["events"]
        .as_array_mut()
        .unwrap()
        .extend([
            serde_json::json!({ "event": "focus", "action": "focus-name" }),
            serde_json::json!({ "event": "ime-cancel", "action": "cancel-name" }),
        ]);
    let document =
        NativeUiDocument::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    let field_context = UiEventContext {
        target: UiKey::new(3, 1),
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "ime-host",
            [
                UiPermission::Focus,
                UiPermission::TextEdit,
                UiPermission::Ime,
            ],
        ),
    };
    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        &mut |_| {},
    )
    .unwrap();
    let button = document.node_for_key(UiKey::new(4, 1)).unwrap();
    host.typed_text_input(OWNER, 2, generation, &document, &field_context, field, "!")
        .unwrap();
    let button_without_text_commit = UiEventContext {
        target: UiKey::new(4, 1),
        permission_context: UiPermissionContext::new(
            "focus-ime-only",
            [UiPermission::Focus, UiPermission::Ime],
        ),
        ..field_context.clone()
    };
    assert!(matches!(
        host.typed_focus(
            OWNER,
            2,
            generation,
            &document,
            &button_without_text_commit,
            button,
            &mut |_| {},
        ),
        Err(TypedUiTargetError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        Some(UiKey::new(3, 1))
    );
    host.typed_set_preedit(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        Some("中"),
        None,
    )
    .unwrap();

    let button_without_ime = UiEventContext {
        target: UiKey::new(4, 1),
        permission_context: UiPermissionContext::new("focus-only", [UiPermission::Focus]),
        ..field_context.clone()
    };
    assert!(matches!(
        host.typed_focus(
            OWNER,
            2,
            generation,
            &document,
            &button_without_ime,
            button,
            &mut |_| {},
        ),
        Err(TypedUiTargetError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        Some(UiKey::new(3, 1))
    );
    let button_context = UiEventContext {
        target: UiKey::new(4, 1),
        ..field_context.clone()
    };
    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &button_context,
        button,
        &mut |_| {},
    )
    .unwrap();
    let no_ime = UiEventContext {
        permission_context: UiPermissionContext::new("focus-only", [UiPermission::Focus]),
        ..field_context.clone()
    };
    assert!(matches!(
        host.typed_take_edit_events(OWNER, &document, &no_ime, false),
        Err(TypedUiEditError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    let events = host
        .typed_take_edit_events(OWNER, &document, &field_context, false)
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].target, UiKey::new(3, 1));
    assert_eq!(
        events[0].kind,
        SceneEditEventKind::Changed("Alice!".into())
    );
    assert_eq!(
        events[1].kind,
        SceneEditEventKind::Commit("Alice!".into())
    );
    assert_eq!(
        events[1]
            .admitted
            .as_ref()
            .map(|event| (event.event, event.action.as_str())),
        Some((UiEventKind::TextCommit, "name"))
    );
    assert_eq!(
        events[2].kind,
        SceneEditEventKind::CompositionCancelled
    );
    assert_eq!(
        events[2]
            .admitted
            .as_ref()
            .map(|event| (event.event, event.action.as_str())),
        Some((UiEventKind::ImeCancel, "cancel-name"))
    );

    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        &mut |_| {},
    )
    .unwrap();
    host.typed_set_preedit(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        Some("组"),
        None,
    )
    .unwrap();
    let button_bounds = host
        .typed_node_bounds(OWNER, 2, generation, &document, UiKey::new(4, 1))
        .unwrap()
        .unwrap();
    let pointer_without_ime = UiEventContext {
        target: UiKey::new(4, 1),
        permission_context: UiPermissionContext::new("pointer-only", [UiPermission::Pointer]),
        ..field_context.clone()
    };
    assert!(matches!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &pointer_without_ime,
            PointerAction::Down,
            button_bounds.x() + button_bounds.width() / 2.0,
            button_bounds.y() + button_bounds.height() / 2.0,
            true,
            &mut |_| EventReply::default(),
            &mut |_| {},
        ),
        Err(TypedUiPointerError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        Some(UiKey::new(3, 1))
    );
    assert!(matches!(
        host.typed_clear_focus(
            OWNER,
            2,
            generation,
            &document,
            &no_ime,
            &mut |_| {},
        ),
        Err(TypedUiTargetError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        Some(UiKey::new(3, 1))
    );
    let mut cleared = Vec::new();
    assert_eq!(
        host.typed_clear_focus(
            OWNER,
            2,
            generation,
            &document,
            &field_context,
            &mut |event| cleared.push(event),
        )
        .unwrap(),
        Some(UiKey::new(3, 1))
    );
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        None
    );
    assert_eq!(
        cleared,
        vec![jasper_ui_engine::native::TypedUiFocusEvent {
            target: UiKey::new(3, 1),
            focused: false,
            admitted: Some(jasper_ui_engine::ui_ir::AdmittedEvent {
                target: UiKey::new(3, 1),
                event: UiEventKind::Focus,
                action: "focus-name".into(),
            }),
        }]
    );
    let events = host
        .typed_take_edit_events(OWNER, &document, &field_context, false)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].kind,
        SceneEditEventKind::CompositionCancelled
    );

    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        &mut |_| {},
    )
    .unwrap();
    host.typed_set_preedit(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        Some("界"),
        None,
    )
    .unwrap();
    assert!(matches!(
        host.typed_deactivate(
            OWNER,
            2,
            generation,
            &document,
            &no_ime,
            &mut |_| {},
        ),
        Err(TypedUiTargetError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        Some(UiKey::new(3, 1))
    );
    let mut focus_out = Vec::new();
    assert_eq!(
        host.typed_deactivate(
            OWNER,
            2,
            generation,
            &document,
            &field_context,
            &mut |event| focus_out.push(event),
        )
        .unwrap(),
        Some(UiKey::new(3, 1))
    );
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        None
    );
    assert_eq!(
        focus_out,
        vec![jasper_ui_engine::native::TypedUiFocusEvent {
            target: UiKey::new(3, 1),
            focused: false,
            admitted: Some(jasper_ui_engine::ui_ir::AdmittedEvent {
                target: UiKey::new(3, 1),
                event: UiEventKind::Focus,
                action: "focus-name".into(),
            }),
        }]
    );
    let events = host
        .typed_take_edit_events(OWNER, &document, &field_context, false)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].kind,
        SceneEditEventKind::CompositionCancelled
    );
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn typed_pointer_routes_and_activates_by_ui_key_across_an_identity_preserving_move() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{
        EventReply, PointerAction, SceneEventKind, TypedUiPointerError,
    };

    const OWNER: u64 = 86;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"][0]["node"]["children"][2]["events"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "event": "hover",
            "action": "preview-save"
        }));
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut document = NativeUiDocument::from_json_slice(&bytes).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let button = UiKey::new(4, 1);
    let button_node = document.node_for_key(button).unwrap();
    let bounds = host
        .typed_node_bounds(OWNER, 2, generation, &document, button)
        .unwrap()
        .unwrap();
    let (x, y) = (
        bounds.x() + bounds.width() / 2.0,
        bounds.y() + bounds.height() / 2.0,
    );
    assert_eq!(
        host.typed_pointer_target(OWNER, 2, generation, &document, x, y)
            .unwrap(),
        Some(button)
    );
    assert!(matches!(
        host.typed_pointer_target(OWNER, 2, generation, &document, f32::NAN, y),
        Err(TypedUiPointerError::InvalidPoint)
    ));
    assert!(matches!(
        host.typed_pointer_target(OWNER, 3, generation, &document, x, y),
        Err(TypedUiPointerError::RevisionMismatch { .. })
    ));
    assert!(matches!(
        host.typed_pointer_target(OWNER, 2, generation + 1, &document, x, y),
        Err(TypedUiPointerError::HostRejected)
    ));

    let context = UiEventContext {
        target: button,
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "pointer-host",
            [UiPermission::Pointer, UiPermission::Activate],
        ),
    };
    let mut hover = Vec::new();
    assert_eq!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &context,
            PointerAction::Move,
            x,
            y,
            false,
            &mut |event| {
                hover.push(event);
                EventReply::default()
            },
            &mut |_| {},
        )
        .unwrap(),
        Some(button)
    );
    let hover = hover
        .into_iter()
        .find(|event| event.kind == SceneEventKind::Pointer(PointerAction::Move))
        .expect("pointer move is projected to the hover target");
    assert_eq!(hover.target, button);
    assert_eq!(
        hover
            .admitted
            .as_ref()
            .map(|event| (event.event, event.action.as_str())),
        Some((UiEventKind::Hover, "preview-save"))
    );
    let mut down = Vec::new();
    let mut focus = Vec::new();
    assert_eq!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &context,
            PointerAction::Down,
            x,
            y,
            true,
            &mut |event| {
                down.push(event);
                EventReply::default()
            },
            &mut |event| focus.push(event),
        )
        .unwrap(),
        Some(button)
    );
    assert!(down.iter().all(|event| event.target == button));
    assert!(down
        .iter()
        .any(|event| event.kind == SceneEventKind::Pointer(PointerAction::Down)));
    assert_eq!(
        focus,
        vec![jasper_ui_engine::native::TypedUiFocusEvent {
            target: button,
            focused: true,
            admitted: None,
        }]
    );

    let wrong_target = UiEventContext {
        target: UiKey::new(2, 1),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &wrong_target,
            PointerAction::Move,
            x,
            y,
            true,
            &mut |_| EventReply::default(),
            &mut |_| {},
        ),
        Err(TypedUiPointerError::Admission(_))
    ));
    let no_pointer = UiEventContext {
        permission_context: UiPermissionContext::new("activate-only", [UiPermission::Activate]),
        ..context.clone()
    };
    assert!(matches!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &no_pointer,
            PointerAction::Move,
            x,
            y,
            true,
            &mut |_| EventReply::default(),
            &mut |_| {},
        ),
        Err(TypedUiPointerError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::CapabilityDenied { .. }
        ))
    ));

    let mut successor: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    successor["expected_revision"] = serde_json::json!(2);
    successor["next_revision"] = serde_json::json!(3);
    successor["ops"] = serde_json::json!([{
        "op": "move",
        "key": { "semantic_id": 4, "generation": 1 },
        "parent": { "semantic_id": 1, "generation": 1 },
        "index": 1
    }]);
    let successor = UiFrame::from_json_slice(&serde_json::to_vec(&successor).unwrap()).unwrap();
    host.try_apply_ui_frame(OWNER, &mut document, &successor)
        .unwrap();
    assert_eq!(document.revision(), 3);
    assert_eq!(document.node_for_key(button), Some(button_node));
    let moved = host
        .typed_node_bounds(OWNER, 3, generation, &document, button)
        .unwrap()
        .unwrap();
    let (x, y) = (
        moved.x() + moved.width() / 2.0,
        moved.y() + moved.height() / 2.0,
    );
    assert_eq!(
        host.typed_pointer_target(OWNER, 3, generation, &document, x, y)
            .unwrap(),
        Some(button)
    );
    assert!(matches!(
        host.typed_pointer(
            OWNER,
            3,
            generation,
            &document,
            &context,
            PointerAction::Move,
            x,
            y,
            true,
            &mut |_| EventReply::default(),
            &mut |_| {},
        ),
        Err(TypedUiPointerError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::StaleRevision { .. }
        ))
    ));

    let context = UiEventContext {
        frame_revision: 3,
        ..context
    };
    let mut up = Vec::new();
    assert_eq!(
        host.typed_pointer(
            OWNER,
            3,
            generation,
            &document,
            &context,
            PointerAction::Up,
            x,
            y,
            false,
            &mut |event| {
                up.push(event);
                EventReply::default()
            },
            &mut |_| {},
        )
        .unwrap(),
        Some(button)
    );
    let activation = up
        .iter()
        .find(|event| event.kind == SceneEventKind::Activate)
        .expect("completed click activates the same Typed target");
    assert_eq!(activation.target, button);
    assert_eq!(
        activation.admitted.as_ref().map(|event| event.action.as_str()),
        Some("save")
    );
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn typed_pointer_focus_pre_admission_is_limited_to_down_without_blocking_hover() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{EventReply, PointerAction, TypedUiPointerError};

    const OWNER: u64 = 91;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"][0]["node"]["children"][2]["events"] = serde_json::json!([
        { "event": "click", "action": "save" },
        { "event": "focus", "action": "focus-save" }
    ]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let button = UiKey::new(4, 1);
    let bounds = host
        .typed_node_bounds(OWNER, 2, generation, &document, button)
        .unwrap()
        .unwrap();
    let (x, y) = (
        bounds.x() + bounds.width() / 2.0,
        bounds.y() + bounds.height() / 2.0,
    );
    let context = UiEventContext {
        target: button,
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "pointer-only",
            [UiPermission::Pointer, UiPermission::Activate],
        ),
    };
    assert_eq!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &context,
            PointerAction::Move,
            x,
            y,
            false,
            &mut |_| EventReply::default(),
            &mut |_| {},
        )
        .unwrap(),
        Some(button)
    );
    assert_eq!(host.typed_focused_node(OWNER, 2, &document).unwrap(), None);
    assert!(matches!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &context,
            PointerAction::Down,
            x,
            y,
            true,
            &mut |_| EventReply::default(),
            &mut |_| {},
        ),
        Err(TypedUiPointerError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    assert_eq!(host.typed_focused_node(OWNER, 2, &document).unwrap(), None);
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
    let mut activations = Vec::new();
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
            &mut |event| activations.push(event),
        )
        .unwrap()
        .unwrap();
    assert_eq!(admitted.action, "keyboard-save");
    assert_eq!(
        activations,
        vec![jasper_ui_engine::native::TypedUiActivationEvent {
            target: UiKey::new(4, 1),
        }]
    );
    let mut host_activation = Vec::new();
    let admitted = host
        .typed_activate(
            OWNER,
            2,
            generation,
            &document,
            &context,
            button,
            &mut |event| host_activation.push(event),
        )
        .unwrap();
    assert_eq!(admitted.action, "keyboard-save");
    assert_eq!(
        host_activation,
        vec![jasper_ui_engine::native::TypedUiActivationEvent {
            target: UiKey::new(4, 1),
        }]
    );

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

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn typed_scroll_event_requires_current_surface_key_delta_and_binding_permission() {
    use jasper_ui_engine::geometry::Point;
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::TypedUiScrollError;

    const OWNER: u64 = 87;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("settings-page.ir.json")).unwrap();
    value["ops"][0]["node"]["events"] = serde_json::json!([{
        "event": "scroll",
        "action": "scroll-settings"
    }]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let host = HostRenderer::new_ui_document(OWNER, &document, &font, 480, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let root = document.node_for_key(UiKey::new(1, 1)).unwrap();
    let context = UiEventContext {
        target: UiKey::new(1, 1),
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new("scroll-host", [UiPermission::Scroll]),
    };
    let event = host
        .typed_scroll_event(
            OWNER,
            2,
            generation,
            &document,
            &context,
            root,
            Point::new(4.0, 12.0),
        )
        .unwrap();
    assert_eq!(event.target, UiKey::new(1, 1));
    assert_eq!(event.delta, Point::new(4.0, 12.0));
    assert_eq!(event.admitted.action, "scroll-settings");
    assert_eq!(
        host.typed_scroll_event(
            OWNER,
            2,
            generation,
            &document,
            &context,
            root,
            Point::ZERO,
        ),
        Err(TypedUiScrollError::InvalidDelta)
    );
    let no_scroll = UiEventContext {
        permission_context: UiPermissionContext::new("pointer-only", [UiPermission::Pointer]),
        ..context
    };
    assert!(matches!(
        host.typed_scroll_event(
            OWNER,
            2,
            generation,
            &document,
            &no_scroll,
            root,
            Point::new(0.0, 1.0),
        ),
        Err(TypedUiScrollError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
}

#[cfg(all(feature = "native-blitz-cpu-pending", feature = "accesskit-pending"))]
#[test]
fn typed_accesskit_actions_require_ui_key_and_permissions() {
    use jasper_ui_engine::native::host::accesskit::{
        Action, ActionRequest, NodeId as KitId, TreeId,
    };
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{
        SceneEditEventKind, TypedAccessibilityAction, TypedUiAccessibilityActionError,
    };

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
    value["ops"][0]["node"]["children"][1]["events"]
        .as_array_mut()
        .unwrap()
        .extend([
            serde_json::json!({ "event": "focus", "action": "focus-name" }),
            serde_json::json!({ "event": "ime-cancel", "action": "cancel-name" }),
        ]);
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

    let mut activated = Vec::new();
    let result = host
        .typed_accesskit_action(
            OWNER,
            2,
            generation,
            &document,
            &context,
            &request(Action::Click),
            &mut |_| {},
            &mut |event| activated.push(event),
        )
        .unwrap();
    assert!(matches!(
        result,
        TypedAccessibilityAction::Activated(ref event) if event.action == "save-accessible"
    ));
    assert_eq!(
        activated,
        vec![jasper_ui_engine::native::TypedUiActivationEvent {
            target: UiKey::new(4, 1),
        }]
    );

    let field = document.node_for_key(UiKey::new(3, 1)).unwrap();
    let field_context = UiEventContext {
        target: UiKey::new(3, 1),
        permission_context: UiPermissionContext::new(
            "field-ime-host",
            [UiPermission::Focus, UiPermission::Ime],
        ),
        ..context.clone()
    };
    host.typed_focus(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        &mut |_| {},
    )
    .unwrap();
    host.typed_set_preedit(
        OWNER,
        2,
        generation,
        &document,
        &field_context,
        field,
        Some("中"),
        None,
    )
    .unwrap();
    assert!(matches!(
        host.typed_accesskit_action(
            OWNER,
            2,
            generation,
            &document,
            &context,
            &request(Action::Focus),
            &mut |_| {},
            &mut |_| {},
        ),
        Err(TypedUiAccessibilityActionError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    assert_eq!(
        host.typed_focused_node(OWNER, 2, &document).unwrap(),
        Some(UiKey::new(3, 1))
    );
    let context_with_ime = UiEventContext {
        permission_context: UiPermissionContext::new(
            "accessibility-ime-host",
            [
                UiPermission::AccessibilityAction,
                UiPermission::Focus,
                UiPermission::Ime,
            ],
        ),
        ..context.clone()
    };
    let mut focused = Vec::new();
    let result = host
        .typed_accesskit_action(
            OWNER,
            2,
            generation,
            &document,
            &context_with_ime,
            &request(Action::Focus),
            &mut |event| focused.push(event),
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(result, TypedAccessibilityAction::Focused(UiKey::new(4, 1)));
    assert_eq!(
        focused,
        vec![
            jasper_ui_engine::native::TypedUiFocusEvent {
                target: UiKey::new(3, 1),
                focused: false,
                admitted: Some(jasper_ui_engine::ui_ir::AdmittedEvent {
                    target: UiKey::new(3, 1),
                    event: UiEventKind::Focus,
                    action: "focus-name".into(),
                }),
            },
            jasper_ui_engine::native::TypedUiFocusEvent {
                target: UiKey::new(4, 1),
                focused: true,
                admitted: None,
            },
        ]
    );
    let events = host
        .typed_take_edit_events(OWNER, &document, &field_context, false)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].kind,
        SceneEditEventKind::CompositionCancelled
    );

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
    assert!(matches!(
        host.typed_accesskit_action(
            OWNER,
            3,
            generation,
            &document,
            &context_with_ime,
            &request(Action::Click),
            &mut |_| {},
            &mut |_| {},
        ),
        Err(TypedUiAccessibilityActionError::RevisionMismatch { .. })
    ));
    assert!(matches!(
        host.typed_accesskit_action(
            OWNER,
            2,
            generation + 1,
            &document,
            &context_with_ime,
            &request(Action::Click),
            &mut |_| {},
            &mut |_| {},
        ),
        Err(TypedUiAccessibilityActionError::HostRejected)
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
fn typed_table_metrics_project_table_and_cell_identities() {
    use jasper_ui_engine::native::host::HostRenderer;

    const OWNER: u64 = 89;
    let document = NativeUiDocument::from_json_slice(&fixture("data-table.ir.json")).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let host = HostRenderer::new_ui_document(OWNER, &document, &font, 640, 320).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let metrics = host
        .typed_table_metrics(OWNER, 2, generation, &document, UiKey::new(20, 1))
        .unwrap()
        .unwrap();
    assert_eq!(metrics.target, UiKey::new(20, 1));
    assert_eq!(
        metrics
            .cells
            .iter()
            .map(|cell| cell.target)
            .collect::<Vec<_>>(),
        vec![UiKey::new(22, 1), UiKey::new(23, 1)]
    );
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
fn typed_overlay_info_and_dismiss_request_use_ui_keys_without_promoting_candidate_status() {
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::native::{
        EventReply, PointerAction, TypedUiOverlayError,
    };

    const OWNER: u64 = 88;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("dialog-overlay.ir.json")).unwrap();
    value["ops"][0]["node"]["children"][1]["events"] = serde_json::json!([{
        "event": "dismiss",
        "action": "dismiss-delete"
    }]);
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let mut host = HostRenderer::new_ui_document(OWNER, &document, &font, 640, 480).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let overlays = host.typed_overlays(OWNER, 2, generation, &document).unwrap();
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].target, UiKey::new(32, 1));
    assert_eq!(
        document
            .mapped_properties()
            .get("overlay.layer")
            .map(|property| (property.support, property.host_accepted)),
        Some((SupportLevel::Candidate, false))
    );
    let placement = overlays[0].placement.unwrap();
    let (x, y) = if !placement.rect.contains(jasper_ui_engine::geometry::Point::new(1.0, 1.0)) {
        (1.0, 1.0)
    } else {
        (639.0, 479.0)
    };
    let context = UiEventContext {
        target: UiKey::new(30, 1),
        identity_domain: "orders-app/window-main".into(),
        owner: "desktop-host".into(),
        session: "session-2026-11-17-1".into(),
        frame_revision: 2,
        permission_context: UiPermissionContext::new(
            "overlay-host",
            [UiPermission::Pointer, UiPermission::Dismiss],
        ),
    };
    assert_eq!(
        host.typed_pointer(
            OWNER,
            2,
            generation,
            &document,
            &context,
            PointerAction::Down,
            x,
            y,
            true,
            &mut |_| EventReply::default(),
            &mut |_| {},
        )
        .unwrap(),
        None
    );
    let events = host
        .typed_take_overlay_events(OWNER, &document, &context)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, UiKey::new(32, 1));
    assert_eq!(
        events[0]
            .admitted
            .as_ref()
            .map(|event| event.action.as_str()),
        Some("dismiss-delete")
    );

    let no_dismiss = UiEventContext {
        permission_context: UiPermissionContext::new("pointer-only", [UiPermission::Pointer]),
        ..context.clone()
    };
    host.typed_pointer(
        OWNER,
        2,
        generation,
        &document,
        &no_dismiss,
        PointerAction::Down,
        x,
        y,
        true,
        &mut |_| EventReply::default(),
        &mut |_| {},
    )
    .unwrap();
    assert!(matches!(
        host.typed_take_overlay_events(OWNER, &document, &no_dismiss),
        Err(TypedUiOverlayError::Admission(
            jasper_ui_engine::ui_ir::UiEventAdmissionError::PermissionDenied { .. }
        ))
    ));
    let events = host
        .typed_take_overlay_events(OWNER, &document, &context)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, UiKey::new(32, 1));
}

#[cfg(feature = "native-blitz-cpu-pending")]
#[test]
fn v0_flex_wrap_shrink_alignment_and_text_direction_execute_in_native_layout() {
    use jasper_ui_engine::native::host::SceneHostView;
    use jasper_ui_engine::style::{Align, FlexWrap, Justify, TextAlign, TextDirection};

    let value = serde_json::json!({
        "ir_version": "0.1.0",
        "identity_domain": "layout-app/window-main",
        "owner": "desktop-host",
        "session": "session-layout-1",
        "expected_revision": 1,
        "next_revision": 2,
        "ops": [{
            "op": "insert",
            "parent": null,
            "index": 0,
            "node": {
                "key": { "semantic_id": 1, "generation": 1 },
                "element": { "kind": "container" },
                "semantics": { "role": "form", "label": "Layout" },
                "style": {
                    "box": {
                        "width": { "unit": "px", "value": 140 },
                        "height": { "unit": "px", "value": 100 }
                    },
                    "flex": {
                        "direction": "row",
                        "wrap": "wrap",
                        "align_items": "baseline",
                        "justify_content": "space-evenly"
                    }
                },
                "children": [
                    {
                        "key": { "semantic_id": 2, "generation": 1 },
                        "element": { "kind": "container" },
                        "semantics": { "role": "container", "label": "First" },
                        "style": {
                            "box": {
                                "width": { "unit": "px", "value": 80 },
                                "height": { "unit": "px", "value": 20 }
                            },
                            "flex": { "shrink": 0 }
                        }
                    },
                    {
                        "key": { "semantic_id": 3, "generation": 1 },
                        "element": { "kind": "container" },
                        "semantics": { "role": "container", "label": "Second" },
                        "style": {
                            "box": {
                                "width": { "unit": "px", "value": 80 },
                                "height": { "unit": "px", "value": 20 }
                            },
                            "flex": { "shrink": 0 }
                        }
                    },
                    {
                        "key": { "semantic_id": 4, "generation": 1 },
                        "element": { "kind": "text", "content": "Hello" },
                        "semantics": { "role": "text", "label": "Hello" },
                        "style": {
                            "box": {
                                "width": { "unit": "px", "value": 80 },
                                "height": { "unit": "px", "value": 20 }
                            },
                            "typography": { "align": "end", "direction": "rtl" }
                        }
                    }
                ]
            }
        }]
    });
    let frame = UiFrame::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    let document = NativeUiDocument::from_frame(&frame).unwrap();
    let root = document
        .scene()
        .find(document.node_for_key(UiKey::new(1, 1)).unwrap())
        .unwrap();
    assert_eq!(root.style.flex_wrap, FlexWrap::Wrap);
    assert_eq!(root.style.align_items, Align::Baseline);
    assert_eq!(root.style.justify_content, Justify::SpaceEvenly);
    let text = document
        .scene()
        .find(document.node_for_key(UiKey::new(4, 1)).unwrap())
        .unwrap();
    assert_eq!(text.style.text_align, TextAlign::End);
    assert_eq!(text.style.text_direction, TextDirection::Rtl);

    let font = std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .unwrap();
    let view = SceneHostView::new(90, document.scene(), &font, 140, 100).unwrap();
    let first = view
        .node_bounds(90, 2, document.node_for_key(UiKey::new(2, 1)).unwrap())
        .unwrap();
    let second = view
        .node_bounds(90, 2, document.node_for_key(UiKey::new(3, 1)).unwrap())
        .unwrap();
    assert!(first.width() > 79.0 && second.width() > 79.0);
    assert!(
        second.y() >= first.y() + 19.0,
        "wrap and shrink=0 place the second 80px child on the next line: {first:?} {second:?}"
    );
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
