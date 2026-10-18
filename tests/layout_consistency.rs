//! Cross-host layout consistency samples.
//! These lock the portable PSS subset (percent/min-max/gap/alignment) to
//! concrete geometry so every host sees the same result from Taffy.

use jasper_ui_engine::geometry::{Edges, Size};
use jasper_ui_engine::layout::{LayoutEngine, LayoutTree, TaffyLayoutEngine};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Align, FlexDirection, Justify, Length, Style};
use jasper_ui_engine::text::SimpleTextEngine;

fn layout(scene: &Scene) -> LayoutTree {
    TaffyLayoutEngine
        .layout(scene, Size::new(200.0, 100.0), &SimpleTextEngine)
        .expect("layout")
}

fn root(children: Vec<Node>) -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(200.0), Length::Px(100.0))
                    .direction(FlexDirection::Row),
            )
            .with_children(children),
    )
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn percentage_sizes_resolve_against_the_content_box() {
    let scene = root(vec![Node::rect(NodeId(2)).with_style(
        Style::default().size(Length::Percent(50.0), Length::Percent(25.0)),
    )]);
    let tree = layout(&scene);
    let child = tree.find(NodeId(2)).unwrap();
    close(child.rect.width(), 100.0);
    close(child.rect.height(), 25.0);
}

#[test]
fn min_and_max_constraints_clamp_percentage_sizes() {
    let mut style = Style::default().size(Length::Percent(80.0), Length::Percent(10.0));
    style.max_width = Length::Px(120.0);
    style.min_height = Length::Px(30.0);
    let scene = root(vec![Node::rect(NodeId(2)).with_style(style)]);
    let tree = layout(&scene);
    let child = tree.find(NodeId(2)).unwrap();
    close(child.rect.width(), 120.0);
    close(child.rect.height(), 30.0);
}

#[test]
fn padding_and_gap_are_applied_in_row_order() {
    let scene = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(200.0), Length::Px(100.0))
                    .direction(FlexDirection::Row)
                    .padding(Edges::all(5.0))
                    .gap(10.0),
            )
            .child(
                Node::rect(NodeId(2))
                    .with_style(Style::default().size(Length::Px(40.0), Length::Px(20.0))),
            )
            .child(
                Node::rect(NodeId(3))
                    .with_style(Style::default().size(Length::Px(40.0), Length::Px(20.0))),
            ),
    );
    let tree = layout(&scene);
    let first = tree.find(NodeId(2)).unwrap();
    let second = tree.find(NodeId(3)).unwrap();
    close(first.rect.x(), 5.0);
    close(first.rect.y(), 5.0);
    close(second.rect.x(), 55.0);
    close(second.rect.y(), 5.0);
}

#[test]
fn center_alignment_places_a_fixed_child_in_the_middle() {
    let scene = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(200.0), Length::Px(100.0))
                    .direction(FlexDirection::Row)
                    .justify_content(Justify::Center)
                    .align_items(Align::Center),
            )
            .child(
                Node::rect(NodeId(2))
                    .with_style(Style::default().size(Length::Px(50.0), Length::Px(20.0))),
            ),
    );
    let tree = layout(&scene);
    let child = tree.find(NodeId(2)).unwrap();
    close(child.rect.x(), 75.0);
    close(child.rect.y(), 40.0);
}

#[test]
fn flex_grow_consumes_free_space_without_moving_the_next_sibling() {
    let scene = root(vec![
        Node::rect(NodeId(2)).with_style(
            Style::default()
                .size(Length::Px(50.0), Length::Px(20.0))
                .flex_grow(1.0),
        ),
        Node::rect(NodeId(3)).with_style(Style::default().size(Length::Px(50.0), Length::Px(20.0))),
    ]);
    let tree = layout(&scene);
    let first = tree.find(NodeId(2)).unwrap();
    let second = tree.find(NodeId(3)).unwrap();
    close(first.rect.width(), 150.0);
    close(second.rect.x(), 150.0);
    close(second.rect.width(), 50.0);
}
