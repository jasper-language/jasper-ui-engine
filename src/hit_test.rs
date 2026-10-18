//! Hit testing over a laid-out scene.
//! 只依赖布局树；命中顺序与绘制顺序一致：后绘制的节点在上层。

use crate::geometry::Point;
use crate::layout::{LayoutBox, LayoutTree};
use crate::scene::NodeId;
use crate::style::Display;

/// Returns the topmost painted node containing `point`.
pub fn hit_test(layout: &LayoutTree, point: Point) -> Option<NodeId> {
    layout
        .boxes
        .iter()
        .rev()
        .find(|item| is_hit_testable(item) && item.rect.contains(point))
        .map(|item| item.node_id)
}

/// Returns the root-to-target path for event bubbling. Empty when nothing is hit.
pub fn hit_test_path(layout: &LayoutTree, point: Point) -> Vec<NodeId> {
    let Some(target) = hit_test(layout, point) else {
        return Vec::new();
    };
    let mut path = Vec::new();
    let mut current = Some(target);
    while let Some(id) = current {
        path.push(id);
        current = layout.find(id).and_then(|item| item.parent);
    }
    path.reverse();
    path
}

/// A transparent layout container is not a hit target by itself; its painted
/// children can still be hit. This keeps container padding from swallowing
/// clicks meant for a child.
fn is_hit_testable(item: &LayoutBox) -> bool {
    if item.style.display == Display::None {
        return false;
    }
    if item.rect.width() <= 0.0 || item.rect.height() <= 0.0 {
        return false;
    }
    item.style.background.a > 0
        || (item.style.border_width > 0.0 && item.style.border_color.a > 0)
        || item.text.as_deref().is_some_and(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Point, Rect, Size};
    use crate::layout::{LayoutBox, LayoutTree};
    use crate::style::{Color, Style};

    fn item(id: u64, parent: Option<u64>, rect: Rect, style: Style) -> LayoutBox {
        LayoutBox {
            node_id: NodeId(id),
            parent: parent.map(NodeId),
            rect,
            content: rect,
            style,
            text: None,
        }
    }

    fn layout() -> LayoutTree {
        LayoutTree {
            viewport: Size::new(100.0, 100.0),
            boxes: vec![
                item(
                    1,
                    None,
                    Rect::new(0.0, 0.0, 100.0, 100.0),
                    Style::default().background(Color::BLACK),
                ),
                item(
                    2,
                    Some(1),
                    Rect::new(10.0, 10.0, 40.0, 40.0),
                    Style::default().background(Color::RED),
                ),
                item(
                    3,
                    Some(1),
                    Rect::new(20.0, 20.0, 40.0, 40.0),
                    Style::default().background(Color::BLUE),
                ),
            ],
        }
    }

    #[test]
    fn picks_topmost_and_builds_path() {
        let tree = layout();
        assert_eq!(hit_test(&tree, Point::new(30.0, 30.0)), Some(NodeId(3)));
        assert_eq!(
            hit_test_path(&tree, Point::new(30.0, 30.0)),
            vec![NodeId(1), NodeId(3)]
        );
        assert_eq!(hit_test(&tree, Point::new(5.0, 5.0)), Some(NodeId(1)));
        assert_eq!(hit_test(&tree, Point::new(200.0, 200.0)), None);
    }

    #[test]
    fn transparent_container_does_not_swallow_children() {
        let tree = LayoutTree {
            viewport: Size::new(100.0, 100.0),
            boxes: vec![
                item(1, None, Rect::new(0.0, 0.0, 100.0, 100.0), Style::default()),
                item(
                    2,
                    Some(1),
                    Rect::new(10.0, 10.0, 20.0, 20.0),
                    Style::default().background(Color::RED),
                ),
            ],
        };
        assert_eq!(hit_test(&tree, Point::new(15.0, 15.0)), Some(NodeId(2)));
        assert_eq!(hit_test(&tree, Point::new(80.0, 80.0)), None);
    }
}
