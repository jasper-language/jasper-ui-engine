//! Migration probe, not accepted production glue. No window or network provider.
use blitz_dom::{DocumentConfig, EventDriver, NoopEventHandler, build_single_font_ctx};
use blitz_traits::events::{BlitzImeEvent, UiEvent};
use blitz_html::HtmlDocument;
use blitz_traits::shell::Viewport;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let font_path = std::env::args().nth(1).ok_or("supply an injected font file")?;
    let bytes = std::fs::read(font_path)?;
    let config = DocumentConfig {
        font_ctx: Some(build_single_font_ctx(&bytes)),
        viewport: Some(Viewport { window_size: (640, 480), ..Viewport::default() }),
        ..DocumentConfig::default()
    };
    let mut document = HtmlDocument::from_html(include_str!("../editor.html"), config);
    document.resolve(0.0);
    let editor = document.query_selector("#editor").map_err(|_| "invalid fixture selector")?.ok_or("missing editor")?;
    if !document.set_focus_to(editor) {
        return Err("fixture could not focus editor".into());
    }
    // Upstream integration diagnostic only: Jasper authorization/cancellation glue is pending.
    EventDriver::new(&mut document, NoopEventHandler)
        .handle_ui_event(UiEvent::Ime(BlitzImeEvent::Commit("提交验证".to_owned())));
    let value = document.get_node(editor)
        .and_then(|node| node.element_data())
        .and_then(|element| element.text_input_data())
        .ok_or("missing upstream text editor")?.editor.raw_text();
    if !value.contains("提交验证") { return Err("IME commit did not reach document".into()); }
    println!("Blitz IME commit reached textarea; display and Jasper isolation acceptance pending");
    Ok(())
}
