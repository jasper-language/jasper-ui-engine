//! Legacy compatibility-only hit testing/capture example: cargo run --example legacy_pointer.
//! 历史兼容演示：旧自有布局/触摸 capture 路径，不是原生主入口；仅保留迁移对照。

use jasper_ui_engine::geometry::{Edges, Point, Size};
use jasper_ui_engine::input::{PointerEvent, PointerId, PointerKind, PointerPhase};
use jasper_ui_engine::layout::{LayoutEngine, TaffyLayoutEngine};
use jasper_ui_engine::pointer::PointerRouter;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use jasper_ui_engine::text::SimpleTextEngine;

fn main() {
    let scene = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(320.0), Length::Px(120.0))
                    .padding(Edges::all(16.0))
                    .gap(12.0)
                    .direction(FlexDirection::Row)
                    .background(Color::rgb(18, 22, 30)),
            )
            .child(
                Node::rect(NodeId(2)).with_style(
                    Style::default()
                        .size(Length::Px(120.0), Length::Px(80.0))
                        .background(Color::GREEN),
                ),
            )
            .child(
                Node::rect(NodeId(3)).with_style(
                    Style::default()
                        .size(Length::Px(120.0), Length::Px(80.0))
                        .background(Color::BLUE),
                ),
            ),
    );

    let layout = TaffyLayoutEngine
        .layout(&scene, Size::new(320.0, 120.0), &SimpleTextEngine)
        .expect("layout");

    let mut router = PointerRouter::new();
    let event = |phase, x, y| PointerEvent {
        pointer_id: PointerId(1),
        kind: PointerKind::Touch,
        phase,
        position: Point::new(x, y),
        ..Default::default()
    };

    for (label, phase, x, y) in [
        ("down-on-green", PointerPhase::Down, 30.0, 30.0),
        ("move-outside", PointerPhase::Move, 300.0, 100.0),
        ("up-outside", PointerPhase::Up, 300.0, 100.0),
        ("hover-blue", PointerPhase::Hover, 200.0, 30.0),
    ] {
        let route = router.route(event(phase, x, y), &layout).expect("route");
        println!(
            "{label}: target={:?} path={:?} captured={}",
            route.target, route.path, route.captured
        );
    }
}
