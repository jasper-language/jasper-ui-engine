//! P0-A native layout coverage: explicit grid tracks/placement, absolute positioning, stacking,
//! flex order and CSS table boxes on the real Blitz path.
//! P0-A 原生布局验收：显式 Grid 轨道/放置、绝对定位、层级、Flex order 和 CSS 表格盒。
#![cfg(feature = "native-engine-cpu")]

use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::geometry::Point;
use jasper_ui_engine::style::{
    Color, Display, GridLine, GridTrack, Inset, Length, Position, Style,
};
use jasper_ui_engine::Engine;

const OWNER: u64 = 64;
const W: u32 = 300;
const H: u32 = 200;

fn render(engine: &mut Engine, version: u64) -> Vec<u8> {
    let mut frame = vec![0u8; (W * H * 4) as usize];
    assert!(engine.render_to_surface(
        OWNER,
        version,
        &mut frame,
        (W * 4) as usize,
        1 << 24
    ));
    frame
}

fn px(frame: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * W as usize + x) * 4;
    frame[i..i + 4].try_into().unwrap()
}

#[test]
fn grid_tracks_placement_absolute_insets_and_stacking_are_real() {
    let root = Style::default()
        .size(Length::Px(W as f32), Length::Px(H as f32))
        .background(Color::WHITE)
        .display(Display::Grid)
        .grid_columns(vec![
            GridTrack::Px(50.0),
            GridTrack::Fr(1.0),
            GridTrack::Percent(20.0),
        ])
        .grid_rows(vec![GridTrack::Px(30.0), GridTrack::Fr(1.0)]);
    let bottom = Style::default()
        .size(Length::Px(20.0), Length::Px(20.0))
        .background(Color::BLUE)
        .position(Position::Absolute)
        .inset(Inset::new(Length::Px(10.0), Length::Auto, Length::Auto, Length::Px(10.0)))
        .z_index(1);
    let top = Style::default()
        .size(Length::Px(10.0), Length::Px(10.0))
        .background(Color::RED)
        .position(Position::Absolute)
        .inset(Inset::new(Length::Px(10.0), Length::Auto, Length::Auto, Length::Px(10.0)))
        .z_index(5);
    let source = Scene::new(
        Node::container(NodeId(1))
            .with_style(root)
            .child(
                Node::container(NodeId(2))
                    .with_style(
                        Style::default()
                            .background(Color::RED)
                            .grid_column(GridLine::Line(1))
                            .grid_row(GridLine::Line(1)),
                    ),
            )
            .child(
                Node::container(NodeId(3))
                    .with_style(
                        Style::default()
                            .background(Color::GREEN)
                            .grid_column(GridLine::Line(2))
                            .grid_row(GridLine::Line(2)),
                    ),
            )
            .child(Node::container(NodeId(4)).with_style(bottom))
            .child(Node::container(NodeId(5)).with_style(top)),
    );
    let mut engine = Engine::new(OWNER, &source, &[], W, H).unwrap();
    let version = source.version();
    let a = engine.node_bounds(OWNER, version, NodeId(2)).unwrap();
    let b = engine.node_bounds(OWNER, version, NodeId(3)).unwrap();
    assert_eq!((a.x(), a.y(), a.width(), a.height()), (0.0, 0.0, 50.0, 30.0));
    assert_eq!((b.x(), b.y(), b.width(), b.height()), (50.0, 30.0, 190.0, 170.0));
    let frame = render(&mut engine, version);
    assert_eq!(px(&frame, 10, 10), [220, 50, 47, 255]);
    assert_eq!(px(&frame, 20, 20), [30, 110, 220, 255]);
}

#[test]
fn flex_order_and_table_display_have_defined_geometry() {
    let flex = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(100.0), Length::Px(20.0))
                    .direction(jasper_ui_engine::style::FlexDirection::Row),
            )
            .child(
                Node::container(NodeId(2))
                    .with_style(
                        Style::default()
                            .size(Length::Px(20.0), Length::Px(20.0))
                            .order(2),
                    ),
            )
            .child(
                Node::container(NodeId(3))
                    .with_style(
                        Style::default()
                            .size(Length::Px(20.0), Length::Px(20.0))
                            .order(1),
                    ),
            )
            .child(
                Node::container(NodeId(4)).with_style(
                    Style::default()
                        .size(Length::Px(20.0), Length::Px(20.0))
                        .position(Position::Relative)
                        .inset(Inset::new(
                            Length::Px(5.0),
                            Length::Auto,
                            Length::Auto,
                            Length::Px(7.0),
                        )),
                ),
            ),
    );
    let engine = Engine::new(OWNER, &flex, &[], 100, 20).unwrap();
    assert_eq!(engine.node_bounds(OWNER, flex.version(), NodeId(2)).unwrap().x(), 40.0);
    assert_eq!(engine.node_bounds(OWNER, flex.version(), NodeId(3)).unwrap().x(), 20.0);
    let relative = engine
        .node_bounds(OWNER, flex.version(), NodeId(4))
        .unwrap();
    assert_eq!((relative.x(), relative.y()), (7.0, 5.0));

    let table = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(240.0), Length::Px(40.0))
                    .display(Display::Table),
            )
            .child(
                Node::container(NodeId(2))
                    .with_style(Style::default().display(Display::TableRow))
                    .child(
                        Node::container(NodeId(3))
                            .with_style(
                                Style::default()
                                    .display(Display::TableCell)
                                    .size(Length::Px(80.0), Length::Px(40.0)),
                            ),
                    )
                    .child(
                        Node::container(NodeId(4))
                            .with_style(
                                Style::default()
                                    .display(Display::TableCell)
                                    .size(Length::Px(160.0), Length::Px(40.0)),
                            ),
                    ),
            ),
    );
    let table_engine = Engine::new(OWNER, &table, &[], 240, 40).unwrap();
    let first = table_engine
        .node_bounds(OWNER, table.version(), NodeId(3))
        .unwrap();
    let second = table_engine
        .node_bounds(OWNER, table.version(), NodeId(4))
        .unwrap();
    assert_eq!((first.x(), first.width()), (0.0, 80.0));
    assert_eq!((second.x(), second.width()), (80.0, 160.0));
    let metrics = table_engine
        .table_metrics(OWNER, table.version(), NodeId(1))
        .unwrap();
    assert_eq!(metrics.column_widths, vec![80.0, 160.0]);
    assert_eq!(metrics.row_heights, vec![40.0]);
    assert_eq!(metrics.cells.len(), 2);
    assert_eq!((metrics.cells[1].row, metrics.cells[1].column), (0, 1));

    let mut tall = Style::default().size(Length::Px(40.0), Length::Px(200.0));
    tall.min_height = Length::Px(200.0);
    let scrolled = Scene::new(
        Node::container(NodeId(1))
            .with_style(Style::default().size(Length::Px(40.0), Length::Px(40.0)))
            .child(
                Node::container(NodeId(2))
                    .with_style(
                        Style::default()
                            .size(Length::Px(40.0), Length::Px(40.0))
                            .overflow(jasper_ui_engine::style::Overflow::Scroll),
                    )
                    .scroll(Point::new(0.0, 50.0))
                    .child(
                        Node::container(NodeId(3))
                            .with_style(tall),
                    ),
            ),
    );
    let scroll_engine = Engine::new(OWNER, &scrolled, &[], 40, 40).unwrap();
    let geometry = scroll_engine
        .scroll_geometry(OWNER, scrolled.version(), NodeId(2))
        .unwrap();
    assert_eq!((geometry.viewport.width, geometry.viewport.height), (40.0, 40.0));
    assert_eq!(geometry.content.height, 200.0);
    assert_eq!(geometry.offset, Point::new(0.0, 50.0));
    assert_eq!(geometry.max_offset, Point::new(0.0, 160.0));
    assert_eq!(geometry.visible(5.0), jasper_ui_engine::geometry::Rect::new(0.0, 45.0, 40.0, 50.0));
}
