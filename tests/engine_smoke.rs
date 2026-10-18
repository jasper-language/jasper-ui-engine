//! End-to-end smoke test: scene -> layout -> paint -> batch -> software backend.

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Size;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, Length, Style};
use jasper_ui_engine::text::SimpleTextEngine;

#[test]
fn renders_a_rect_through_the_full_pipeline() {
    let scene = Scene::new(
        Node::rect(NodeId(1)).with_style(
            Style::default()
                .size(Length::Px(32.0), Length::Px(32.0))
                .background(Color::RED),
        ),
    );
    let backend = HeadlessBackend::new(32, 32);
    let mut engine = Engine::new(Size::new(32.0, 32.0), backend, Box::new(SimpleTextEngine))
        .with_clear_color(Color::WHITE);
    let stats = engine.render(&scene).expect("headless render");
    assert_eq!(stats.layout_boxes, 1);
    assert!(stats.paint_ops >= 2);
    assert!(stats.batch_bytes > 10);
    assert_eq!(engine.backend().canvas().pixel(0, 0).unwrap(), Color::RED);
}
