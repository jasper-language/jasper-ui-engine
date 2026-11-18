use jasper_ui_engine::ui_ir::{project_jui2_initial_frame, NativeUiDocument};

fn main() {
    let document = NativeUiDocument::from_json_slice(include_bytes!(
        "../tests/fixtures/ui-ir-v0/settings-page.ir.json"
    ))
    .expect("settings fixture");
    let frame = project_jui2_initial_frame(&document).expect("JUI 2 initial frame");
    print!("{}", frame.text);
    eprintln!("losses={}", frame.losses.len());
    for loss in frame.losses {
        eprintln!("{}: {}", loss.path, loss.detail);
    }
}
