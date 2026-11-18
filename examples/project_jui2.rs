use jasper_ui_engine::ui_ir::{
    project_jui2_initial_frame, project_jui2_scene_update, NativeUiDocument, UiFrame,
};

fn main() {
    let mut successor = false;
    let mut fixture_path = None;
    for arg in std::env::args().skip(1) {
        if arg == "--successor" {
            successor = true;
        } else if fixture_path.replace(arg).is_some() {
            panic!("only one fixture path is accepted");
        }
    }
    let bytes = match fixture_path {
        Some(path) => std::fs::read(path).expect("fixture path"),
        None => include_bytes!("../tests/fixtures/ui-ir-v0/settings-page.ir.json").to_vec(),
    };
    let mut document = NativeUiDocument::from_json_slice(&bytes).expect("UI IR fixture");
    let initial = project_jui2_initial_frame(&document).expect("JUI 2 initial frame");
    print!("{}", initial.text);
    print_losses("initial", &initial.losses);
    if successor {
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("fixture JSON");
        value["expected_revision"] = serde_json::json!(2);
        value["next_revision"] = serde_json::json!(3);
        value["ops"] = serde_json::json!([
            {
                "op": "update",
                "key": { "semantic_id": 2, "generation": 1 },
                "fields": {
                    "element": { "kind": "text", "content": "Updated" },
                    "semantics": { "role": "heading", "label": "Updated", "level": 1 },
                    "style": { "typography": { "size": 20 } }
                }
            },
            {
                "op": "insert",
                "parent": { "semantic_id": 1, "generation": 1 },
                "index": 3,
                "node": {
                    "key": { "semantic_id": 13, "generation": 1 },
                    "element": { "kind": "text", "content": "New" },
                    "semantics": { "role": "text", "label": "New" }
                }
            },
            {
                "op": "move",
                "key": { "semantic_id": 13, "generation": 1 },
                "parent": { "semantic_id": 1, "generation": 1 },
                "index": 0
            },
            {
                "op": "remove",
                "key": { "semantic_id": 4, "generation": 1 }
            }
        ]);
        let encoded = serde_json::to_vec(&value).expect("fixture JSON");
        let frame =
            UiFrame::from_json_slice(&encoded).expect("successor UI IR");
        let update = document
            .apply_frame_with_patches(&frame)
            .expect("successor update");
        let projected =
            project_jui2_scene_update(&document, &update).expect("JUI 2 successor frame");
        print!("{}", projected.text);
        print_losses("successor", &projected.losses);
    }
}

fn print_losses(label: &str, losses: &[jasper_ui_engine::ui_ir::NativeNodeLoss]) {
    eprintln!("{label}_losses={}", losses.len());
    for loss in losses {
        eprintln!("{}: {}", loss.path, loss.detail);
    }
}
