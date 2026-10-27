#![cfg(feature = "skia-text-pending")]
//! Native integration tests, not a replacement for the six-stage acceptance chain.
//! 原生接入测试，不替代六阶段验收。
use jasper_ui_engine::{
    text_skia::{InjectedFont, LocalTextRenderer},
    Color, FlexDirection, Length, Node, NodeId, Scene, Style,
};
fn renderer() -> LocalTextRenderer {
    let latin = std::fs::read(
        std::env::var_os("JASPER_UI_TEST_FONT").expect("explicit test font required"),
    )
    .unwrap();
    let cjk = std::fs::read(
        std::env::var_os("JASPER_UI_TEST_CJK_FONT").expect("explicit CJK font required"),
    )
    .unwrap();
    LocalTextRenderer::new(&[
        InjectedFont {
            bytes: &latin,
            collection_index: None,
        },
        InjectedFont {
            bytes: &cjk,
            collection_index: Some(2),
        },
    ])
    .unwrap()
}
fn scene() -> Scene {
    let text = "Jasper café 中文 مرحبا بالعالم Jasper café 中文 مرحبا بالعالم";
    Scene::try_new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(520.0), Length::Px(420.0))
                    .direction(FlexDirection::Column),
            )
            .child(
                Node::text(NodeId(2), text).with_style(
                    Style::default()
                        .size(Length::Px(120.0), Length::Auto)
                        .text(22.0, Color::BLACK),
                ),
            )
            .child(
                Node::text(NodeId(3), text).with_style(
                    Style::default()
                        .size(Length::Px(280.0), Length::Auto)
                        .text(22.0, Color::BLACK),
                ),
            ),
    )
    .unwrap()
}
#[test]
fn explicit_fonts_required() {
    assert!(LocalTextRenderer::new(&[]).is_err());
    assert!(LocalTextRenderer::new(&[InjectedFont {
        bytes: b"not a font",
        collection_index: None
    }])
    .is_err());
}
#[test]
fn final_widths_and_owned_cpu_frame() {
    let renderer = renderer();
    let scene = scene();
    let frame = renderer
        .prepare(&scene, 520, 420, 520 * 420 * 4, Color::WHITE)
        .unwrap();
    assert!(std::ptr::eq(frame.scene(), &scene));
    let narrow = frame.layout().find(NodeId(2)).unwrap();
    let wide = frame.layout().find(NodeId(3)).unwrap();
    assert_eq!(narrow.content.width(), 120.0);
    assert_eq!(wide.content.width(), 280.0);
    assert!(narrow.content.height() > wide.content.height());
    let pixels = frame.into_rgba().unwrap();
    assert_eq!(pixels.len(), 520 * 420 * 4);
    assert!(
        pixels
            .chunks_exact(4)
            .filter(|p| p[..3] != [255, 255, 255])
            .count()
            > 100
    );
    let repeated = renderer
        .prepare(&scene, 520, 420, 520 * 420 * 4, Color::WHITE)
        .unwrap()
        .into_rgba()
        .unwrap();
    assert_eq!(pixels, repeated);
    if let Some(path) = std::env::var_os("JASPER_UI_TEST_FRAME_OUTPUT") {
        std::fs::write(path, pixels).unwrap();
    }
}
#[test]
fn budget_and_native_bounds_rejected() {
    let r = renderer();
    let s = scene();
    for (w, h, b) in [
        (0, 420, 100),
        (520, 0, 100),
        (520, 420, 100),
        (u32::MAX, 1, usize::MAX),
    ] {
        assert!(r.prepare(&s, w, h, b, Color::WHITE).is_err());
    }
}
#[test]
fn invalid_font_size_rejected() {
    let r = renderer();
    let mut s = scene();
    s.root.children[0].style.font_size = f32::NAN;
    assert!(r
        .prepare(&s, 520, 420, 520 * 420 * 4, Color::WHITE)
        .is_err());
}
#[test]
fn text_height_constraint_mismatch_rejected() {
    let r = renderer();
    let mut s = scene();
    s.root.children[0].style.height = Length::Px(1.0);
    assert!(r
        .prepare(&s, 520, 420, 520 * 420 * 4, Color::WHITE)
        .is_err());
}

#[test]
fn prepared_paragraph_owns_native_font_lifetime() {
    let r = renderer();
    let s = scene();
    let frame = r
        .prepare(&s, 520, 420, 520 * 420 * 4, Color::WHITE)
        .unwrap();
    drop(r);
    assert_eq!(frame.into_rgba().unwrap().len(), 520 * 420 * 4);
}
#[test]
fn invalid_measurement_does_not_call_native_layout() {
    use jasper_ui_engine::TextEngine;
    let r = renderer();
    for (size, width) in [
        (f32::NAN, None),
        (22.0, Some(f32::INFINITY)),
        (-1.0, Some(100.0)),
    ] {
        let m = r.measure("text", size, width);
        assert_eq!(m, jasper_ui_engine::TextMetrics::default());
    }
}

#[test]
fn invalid_native_draw_parameters_rejected_before_frame_creation() {
    use jasper_ui_engine::{canvas::Canvas, Point, Rect};
    let r = renderer();
    for width in [f32::NAN, f32::INFINITY, -1.0] {
        let mut commands = Canvas::new();
        commands.line(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            Color::BLACK,
            width,
        );
        let s = Scene::try_new(
            Node::canvas(NodeId(1), commands)
                .with_style(Style::default().size(Length::Px(64.0), Length::Px(64.0))),
        )
        .unwrap();
        assert!(r.prepare(&s, 64, 64, 64 * 64 * 4, Color::WHITE).is_err());
    }
    let mut commands = Canvas::new();
    commands.fill_rect(Rect::new(f32::MAX, 0.0, f32::MAX, 10.0), Color::BLACK);
    let s = Scene::try_new(
        Node::canvas(NodeId(1), commands)
            .with_style(Style::default().size(Length::Px(64.0), Length::Px(64.0))),
    )
    .unwrap();
    assert!(r.prepare(&s, 64, 64, 64 * 64 * 4, Color::WHITE).is_err());
}
#[test]
fn zero_width_stroke_is_not_a_native_hairline() {
    use jasper_ui_engine::{canvas::Canvas, Point};
    let r = renderer();
    let mut commands = Canvas::new();
    commands.line(
        Point::new(0.0, 0.0),
        Point::new(63.0, 63.0),
        Color::BLACK,
        0.0,
    );
    let s = Scene::try_new(
        Node::canvas(NodeId(1), commands)
            .with_style(Style::default().size(Length::Px(64.0), Length::Px(64.0))),
    )
    .unwrap();
    let pixels = r
        .prepare(&s, 64, 64, 64 * 64 * 4, Color::WHITE)
        .unwrap()
        .into_rgba()
        .unwrap();
    assert!(pixels.chunks_exact(4).all(|p| p == [255, 255, 255, 255]));
}

#[test]
fn hit_testing_uses_prepared_unicode_paragraph_and_own_coordinates() {
    use jasper_ui_engine::Point;
    let r=renderer();
    let s=Scene::try_new(Node::container(NodeId(1)).with_style(Style::default().size(Length::Px(240.0),Length::Px(140.0)).direction(FlexDirection::Column))
        .child(Node::text(NodeId(2),"A😀B").with_style(Style::default().size(Length::Px(120.0),Length::Auto).text(22.0,Color::BLACK)))
        .child(Node::text(NodeId(3),"A😀B").with_style(Style::default().size(Length::Px(200.0),Length::Auto).text(22.0,Color::BLACK)))).unwrap();
    let frame=r.prepare(&s,240,140,240*140*4,Color::WHITE).unwrap();
    let first=frame.layout().find(NodeId(2)).unwrap().content.origin;
    let second=frame.layout().find(NodeId(3)).unwrap().content.origin;
    assert!(second.y>first.y);
    assert_eq!(frame.hit_test_text(NodeId(2),Point::new(first.x-100.0,first.y+5.0)).unwrap().utf16_offset,0);
    assert_eq!(frame.hit_test_text(NodeId(2),Point::new(first.x+1000.0,first.y+5.0)).unwrap().utf16_offset,4);
    assert_eq!(frame.hit_test_text(NodeId(3),Point::new(second.x-100.0,second.y+5.0)).unwrap().utf16_offset,0);
    assert!(frame.hit_test_text(NodeId(999),Point::new(0.0,0.0)).is_none());
    assert!(frame.hit_test_text(NodeId(2),Point::new(f32::NAN,5.0)).is_none());
    assert!(frame.hit_test_text(NodeId(2),Point::new(0.0,f32::INFINITY)).is_none());
    let queried_pixels=frame.into_rgba().unwrap();
    let untouched_pixels=r.prepare(&s,240,140,240*140*4,Color::WHITE).unwrap().into_rgba().unwrap();
    assert_eq!(queried_pixels,untouched_pixels);
}
