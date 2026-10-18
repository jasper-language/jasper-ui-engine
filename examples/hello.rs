//! Minimal headless example. Run with `cargo run --example hello`.
//! 生成 hello.ppm（P6），不依赖窗口/GPU；用于验证布局、绘制、批处理、软件光栅链路。

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::{Edges, Size};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};
use jasper_ui_engine::text::SimpleTextEngine;

fn main() {
    let width = 360.0;
    let height = 220.0;

    let header = Node::rect(NodeId(2)).with_style(
        Style::default()
            .height(Length::Px(36.0))
            .background(Color::rgb(36, 48, 68))
            .border(1.0, Color::rgb(80, 108, 150)),
    );

    let card_style = |color: Color| {
        Style::default()
            .width(Length::Px(150.0))
            .height(Length::Px(110.0))
            .padding(Edges::all(10.0))
            .background(color)
            .border(1.0, Color::WHITE)
    };

    let cards = Node::container(NodeId(3))
        .with_style(
            Style::default()
                .direction(FlexDirection::Row)
                .height(Length::Px(120.0))
                .gap(12.0),
        )
        .child(Node::rect(NodeId(4)).with_style(card_style(Color::rgb(46, 160, 67))))
        .child(Node::rect(NodeId(5)).with_style(card_style(Color::rgb(30, 110, 220))));

    let root = Node::container(NodeId(1))
        .with_style(
            Style::default()
                .size(Length::Px(width), Length::Px(height))
                .padding(Edges::all(16.0))
                .gap(12.0)
                .background(Color::rgb(18, 22, 30)),
        )
        .child(header)
        .child(cards);

    let scene = Scene::new(root);
    let backend = HeadlessBackend::new(width as u32, height as u32);
    let mut engine = Engine::new(
        Size::new(width, height),
        backend,
        Box::new(SimpleTextEngine),
    )
    .with_clear_color(Color::rgb(18, 22, 30));

    match engine.render(&scene) {
        Ok(stats) => {
            let path = "hello.ppm";
            engine.backend().save_ppm(path).expect("write hello.ppm");
            println!(
                "rendered {} layout boxes, {} paint ops, {} batch bytes -> {}",
                stats.layout_boxes, stats.paint_ops, stats.batch_bytes, path
            );
        }
        Err(error) => {
            eprintln!("render failed: {error:?}");
            std::process::exit(1);
        }
    }
}
