//! Pending desktop encapsulation: upstream types stay private to this module.
use blitz_dom::{BaseDocument, DocumentConfig, EventDriver, NoopEventHandler,
    QualName, Namespace, LocalName, build_single_font_ctx};
use blitz_traits::{shell::Viewport, events::{BlitzImeEvent, UiEvent}};

type Error = Box<dyn std::error::Error>;

/// Temporary crate-local surface, not a frozen jasper.ui API.
/// Only trusted synchronous host calls are allowed; ticket admission is still pending.
pub(crate) struct NativeEditor {
    document: BaseDocument,
    editor: blitz_dom::NodeId,
}

impl NativeEditor {
    pub(crate) fn new(font_bytes: &[u8], text: &str, width: u32, height: u32) -> Result<Self, Error> {
        let mut document = BaseDocument::new(DocumentConfig {
            font_ctx: Some(build_single_font_ctx(font_bytes)),
            viewport: Some(Viewport { window_size: (width, height), ..Viewport::default() }),
            ..DocumentConfig::default()
        });
        let root = document.root_node().id;
        let editor;
        {
            let mut tree = document.mutate();
            // These backend element names are private implementation details.
            // Desktop construction never serializes/parses an HTML document.
            let name = |local: &str| QualName::new(None,
                Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from(local));
            let container = tree.create_element(name("html"), Vec::new());
            let body = tree.create_element(name("body"), Vec::new());
            editor = tree.create_element(name("textarea"), Vec::new());
            let content = tree.create_text_node(text);
            tree.append_children(editor, &[content]);
            tree.append_children(body, &[editor]);
            tree.append_children(container, &[body]);
            tree.append_children(root, &[container]);
        }
        document.resolve(0.0);
        if !document.set_focus_to(editor) { return Err("cannot focus native editor".into()); }
        Ok(Self { document, editor })
    }

    pub(crate) fn commit(&mut self, text: &str) {
        EventDriver::new(&mut self.document, NoopEventHandler)
            .handle_ui_event(UiEvent::Ime(BlitzImeEvent::Commit(text.to_owned())));
    }

    pub(crate) fn text(&self) -> Result<&str, Error> {
        Ok(self.document.get_node(self.editor)
            .and_then(|node| node.element_data())
            .and_then(|element| element.text_input_data())
            .ok_or("missing upstream editor")?.editor.raw_text())
    }
}
