//! Layout adapter.
//! PSS 子集 → Taffy；Jasper 公共布局语义不直接暴露 Taffy 类型。

use std::collections::HashMap;

use taffy::geometry::{Rect as TaffyRect, Size as TaffySize};
use taffy::style::{
    AlignItems, BoxSizing, Dimension, Display as TaffyDisplay, FlexDirection as TaffyFlexDirection,
    JustifyContent, LengthPercentage, LengthPercentageAuto, Style as TaffyStyle,
};
use taffy::{AvailableSpace, TaffyTree};

use crate::geometry::{Edges, Point, Rect, Size};
use crate::scene::{Node, NodeId, Scene};
use crate::style::{Align, Display, FlexDirection, Justify, Length, Style};
use crate::text::TextEngine;

/// One node after layout, in absolute viewport coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutBox {
    pub node_id: NodeId,
    pub parent: Option<NodeId>,
    pub rect: Rect,
    pub content: Rect,
    pub style: Style,
    pub text: Option<String>,
    /// Effective ancestor clip (not the node's own overflow clip).
    pub clip: Option<Rect>,
}

/// Result of one layout pass.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTree {
    pub viewport: Size,
    pub boxes: Vec<LayoutBox>,
    /// Scene version this layout was computed from.
    pub scene_version: u64,
}

impl LayoutTree {
    pub fn find(&self, node_id: NodeId) -> Option<&LayoutBox> {
        self.boxes.iter().find(|item| item.node_id == node_id)
    }
}

/// Layout contract. A future Taffy/other adapter can replace the implementation,
/// but the public engine only sees this trait.
pub trait LayoutEngine {
    fn layout(
        &self,
        scene: &Scene,
        viewport: Size,
        text: &dyn TextEngine,
    ) -> Result<LayoutTree, LayoutError>;
}

/// Taffy-backed implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct TaffyLayoutEngine;

#[derive(Clone, Debug)]
struct TextContext {
    text: String,
    font_size: f32,
}

impl LayoutEngine for TaffyLayoutEngine {
    fn layout(
        &self,
        scene: &Scene,
        viewport: Size,
        text: &dyn TextEngine,
    ) -> Result<LayoutTree, LayoutError> {
        let viewport = Size::new(viewport.width.max(0.0), viewport.height.max(0.0));
        let mut tree: TaffyTree<TextContext> = TaffyTree::new();
        let mut taffy_ids: HashMap<NodeId, taffy::NodeId> = HashMap::new();

        let root = build_node(&mut tree, &scene.root, true, viewport, &mut taffy_ids)?;
        tree.compute_layout_with_measure(
            root,
            TaffySize {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
            |inputs, _node_id, context, style| {
                taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        let Some(context) = context else {
                            return TaffySize::ZERO;
                        };
                        let max_width = known.width.or(match available.width {
                            AvailableSpace::Definite(width) => Some(width),
                            AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                        });
                        let metrics = text.measure(&context.text, context.font_size, max_width);
                        TaffySize {
                            width: known.width.unwrap_or(metrics.width),
                            height: known.height.unwrap_or(metrics.height),
                        }
                    },
                )
            },
        )
        .map_err(|error| LayoutError::Taffy(error.to_string()))?;

        let mut boxes = Vec::new();
        collect_boxes(
            &tree,
            &scene.root,
            None,
            None,
            Point::ZERO,
            &taffy_ids,
            &mut boxes,
        )?;
        Ok(LayoutTree {
            viewport,
            boxes,
            scene_version: scene.version(),
        })
    }
}

/// Layout failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    Taffy(String),
    MissingNode(NodeId),
}

fn build_node(
    tree: &mut TaffyTree<TextContext>,
    node: &Node,
    is_root: bool,
    viewport: Size,
    ids: &mut HashMap<NodeId, taffy::NodeId>,
) -> Result<taffy::NodeId, LayoutError> {
    let style = map_style(&node.style, is_root, viewport);
    let taffy_id = match &node.kind {
        crate::scene::NodeKind::Text(content) => tree
            .new_leaf_with_context(
                style,
                TextContext {
                    text: content.clone(),
                    font_size: node.style.font_size,
                },
            )
            .map_err(|error| LayoutError::Taffy(error.to_string()))?,
        _ => {
            let mut children = Vec::with_capacity(node.children.len());
            for child in &node.children {
                children.push(build_node(tree, child, false, viewport, ids)?);
            }
            tree.new_with_children(style, &children)
                .map_err(|error| LayoutError::Taffy(error.to_string()))?
        }
    };
    ids.insert(node.id, taffy_id);
    Ok(taffy_id)
}

fn collect_boxes(
    tree: &TaffyTree<TextContext>,
    node: &Node,
    parent: Option<NodeId>,
    clip: Option<Rect>,
    parent_origin: Point,
    ids: &HashMap<NodeId, taffy::NodeId>,
    out: &mut Vec<LayoutBox>,
) -> Result<(), LayoutError> {
    let taffy_id = *ids.get(&node.id).ok_or(LayoutError::MissingNode(node.id))?;
    let layout = tree
        .layout(taffy_id)
        .map_err(|error| LayoutError::Taffy(error.to_string()))?;
    let origin = Point::new(
        parent_origin.x + layout.location.x,
        parent_origin.y + layout.location.y,
    );
    let rect = Rect::from_origin_size(
        origin,
        Size::new(layout.size.width.max(0.0), layout.size.height.max(0.0)),
    );
    let content = Rect::new(
        origin.x + layout.border.left + layout.padding.left,
        origin.y + layout.border.top + layout.padding.top,
        layout.content_box_width().max(0.0),
        layout.content_box_height().max(0.0),
    );
    out.push(LayoutBox {
        node_id: node.id,
        parent,
        rect,
        content,
        style: node.style.clone(),
        text: node.text_content().map(str::to_owned),
        clip,
    });
    let child_clip = if node.style.overflow == crate::style::Overflow::Visible {
        clip
    } else {
        Some(
            clip.map(|current| current.intersect(content))
                .unwrap_or(content),
        )
    };
    let child_origin = Point::new(origin.x - node.scroll.x, origin.y - node.scroll.y);
    for child in &node.children {
        collect_boxes(
            tree,
            child,
            Some(node.id),
            child_clip,
            child_origin,
            ids,
            out,
        )?;
    }
    Ok(())
}

fn map_style(style: &Style, is_root: bool, viewport: Size) -> TaffyStyle {
    let width = if is_root && matches!(style.width, Length::Auto) {
        Dimension::length(viewport.width.max(0.0))
    } else {
        map_dimension(style.width)
    };
    let height = if is_root && matches!(style.height, Length::Auto) {
        Dimension::length(viewport.height.max(0.0))
    } else {
        map_dimension(style.height)
    };

    TaffyStyle {
        display: match style.display {
            Display::Block => TaffyDisplay::Block,
            Display::Flex => TaffyDisplay::Flex,
            Display::None => TaffyDisplay::None,
        },
        box_sizing: BoxSizing::BorderBox,
        size: TaffySize { width, height },
        min_size: TaffySize {
            width: map_dimension_auto(style.min_width),
            height: map_dimension_auto(style.min_height),
        },
        max_size: TaffySize {
            width: map_dimension_auto(style.max_width),
            height: map_dimension_auto(style.max_height),
        },
        padding: map_edges_lp(style.padding),
        border: map_edges_lp(Edges::all(style.border_width)),
        margin: map_edges_lpa(style.margin),
        gap: TaffySize {
            width: LengthPercentage::length(style.gap.max(0.0)),
            height: LengthPercentage::length(style.gap.max(0.0)),
        },
        flex_direction: match style.direction {
            FlexDirection::Row => TaffyFlexDirection::Row,
            FlexDirection::Column => TaffyFlexDirection::Column,
        },
        align_items: match style.align_items {
            Align::Start => AlignItems::START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::END,
            Align::Stretch => AlignItems::STRETCH,
        },
        justify_content: match style.justify_content {
            Justify::Start => JustifyContent::START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::END,
            Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        },
        flex_grow: style.flex_grow.max(0.0),
        ..Default::default()
    }
}

fn map_dimension(length: Length) -> Dimension {
    match length {
        Length::Auto => Dimension::auto(),
        Length::Px(value) => Dimension::length(value.max(0.0)),
        Length::Percent(value) => Dimension::percent((value / 100.0).max(0.0)),
    }
}

fn map_dimension_auto(length: Length) -> LengthPercentageAuto {
    match length {
        Length::Auto => LengthPercentageAuto::auto(),
        Length::Px(value) => LengthPercentageAuto::length(value.max(0.0)),
        Length::Percent(value) => LengthPercentageAuto::percent((value / 100.0).max(0.0)),
    }
}

fn map_length_percentage(length: Length) -> LengthPercentage {
    match length {
        Length::Auto => LengthPercentage::length(0.0),
        Length::Px(value) => LengthPercentage::length(value.max(0.0)),
        Length::Percent(value) => LengthPercentage::percent((value / 100.0).max(0.0)),
    }
}

fn map_edges_lp(edges: Edges) -> TaffyRect<LengthPercentage> {
    TaffyRect {
        left: map_length_percentage(Length::Px(edges.left)),
        right: map_length_percentage(Length::Px(edges.right)),
        top: map_length_percentage(Length::Px(edges.top)),
        bottom: map_length_percentage(Length::Px(edges.bottom)),
    }
}

fn map_edges_lpa(edges: Edges) -> TaffyRect<LengthPercentageAuto> {
    TaffyRect {
        left: LengthPercentageAuto::length(edges.left.max(0.0)),
        right: LengthPercentageAuto::length(edges.right.max(0.0)),
        top: LengthPercentageAuto::length(edges.top.max(0.0)),
        bottom: LengthPercentageAuto::length(edges.bottom.max(0.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Node;
    use crate::style::{Color, Style};
    use crate::text::SimpleTextEngine;

    fn root_with_children(children: Vec<Node>) -> Scene {
        Scene::new(
            Node::container(NodeId(1))
                .with_style(Style::default().size(Length::Px(200.0), Length::Px(100.0)))
                .with_children(children),
        )
    }

    #[test]
    fn column_layout_uses_gap_and_padding() {
        let scene = root_with_children(vec![
            Node::rect(NodeId(2))
                .with_style(Style::default().size(Length::Px(50.0), Length::Px(20.0))),
            Node::rect(NodeId(3))
                .with_style(Style::default().size(Length::Px(50.0), Length::Px(20.0))),
        ]);
        let root_style = scene.root.style.clone().padding(Edges::all(10.0)).gap(5.0);
        let mut scene = scene;
        scene.root.style = root_style;
        let layout = TaffyLayoutEngine
            .layout(&scene, Size::new(200.0, 100.0), &SimpleTextEngine)
            .unwrap();
        let first = layout.find(NodeId(2)).unwrap();
        let second = layout.find(NodeId(3)).unwrap();
        assert!((first.rect.x() - 10.0).abs() < 0.01);
        assert!((first.rect.y() - 10.0).abs() < 0.01);
        assert!((second.rect.y() - 35.0).abs() < 0.01);
    }

    #[test]
    fn overflow_hidden_clips_and_scrolls_children() {
        let scene = Scene::new(
            Node::container(NodeId(1))
                .with_style(
                    Style::default()
                        .size(Length::Px(100.0), Length::Px(100.0))
                        .overflow(crate::style::Overflow::Hidden),
                )
                .scroll(Point::new(0.0, -20.0))
                .child(
                    Node::rect(NodeId(2))
                        .with_style(Style::default().size(Length::Px(50.0), Length::Px(50.0))),
                ),
        );
        let layout = TaffyLayoutEngine
            .layout(&scene, Size::new(100.0, 100.0), &SimpleTextEngine)
            .unwrap();
        let child = layout.find(NodeId(2)).unwrap();
        assert!(
            (child.rect.y() - 20.0).abs() < 0.01,
            "scroll offset should move children"
        );
        assert_eq!(child.clip, Some(Rect::new(0.0, 0.0, 100.0, 100.0)));
    }

    #[test]
    fn text_measure_participates_in_layout() {
        let scene = Scene::new(
            Node::container(NodeId(1))
                .with_style(Style::default().size(Length::Px(200.0), Length::Px(100.0)))
                .child(
                    Node::text(NodeId(2), "hello")
                        .with_style(Style::default().text(10.0, Color::BLACK)),
                ),
        );
        let layout = TaffyLayoutEngine
            .layout(&scene, Size::new(200.0, 100.0), &SimpleTextEngine)
            .unwrap();
        let text = layout.find(NodeId(2)).unwrap();
        assert!(text.rect.width() > 0.0);
        assert!(text.rect.height() > 0.0);
    }
}
