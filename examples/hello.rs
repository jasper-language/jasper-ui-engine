//! Native CPU example: cargo run --features native-blitz-cpu-pending --example hello -- FONT [OUTPUT].
//! 原生 CPU 候选示例：显式注入字体，经 Blitz 绘制并由宿主写出 PPM；整体验收仍待完成。

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, FlexDirection, Length, Style};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Font acquisition and output paths belong to this embedding host.
    // 字体读取与输出路径属于本嵌入宿主，组件声明不取得系统资源。
    let mut arguments = std::env::args().skip(1);
    let font_path = arguments.next().ok_or("supply an injected font file")?;
    let output = arguments.next().unwrap_or_else(|| "hello.ppm".into());
    if arguments.next().is_some() { return Err("expected FONT [OUTPUT]".into()); }
    let font = std::fs::read(font_path)?;
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
    let mut backend = HeadlessBackend::new(1, 1);
    let mut engine = Engine::new(10, &scene, &font, width as u32, height as u32)
        .ok_or("native scene import rejected")?;
    engine.render_to_backend(10, scene.version(), &mut backend, 4 * 1024 * 1024)
        .map_err(|error| format!("native frame submission failed: {error:?}"))?;
    backend.save_ppm(&output)?;
    if !engine.close(10) { return Err("native view retirement rejected".into()); }
    println!("native CPU frame {}x{} -> {}; acceptance pending", width, height, output);
    Ok(())
}
