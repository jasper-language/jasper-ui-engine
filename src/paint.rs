//! Paint pass: layout boxes -> versioned display list.
//! 绘制阶段只产出与后端无关的显示列表，不直接依赖 GPU/Vello。

use crate::geometry::{Point, Rect};
use crate::layout::LayoutTree;
use crate::style::Color;

/// One backend-independent paint operation.
#[derive(Clone, Debug, PartialEq)]
pub enum PaintOp {
    Clear(Color),
    FillRect {
        rect: Rect,
        color: Color,
    },
    StrokeRect {
        rect: Rect,
        color: Color,
        width: f32,
    },
    DrawText {
        origin: Point,
        text: String,
        font_size: f32,
        color: Color,
    },
    PushClip(Rect),
    PopClip,
}

/// Ordered paint operations for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    pub ops: Vec<PaintOp>,
}

impl DisplayList {
    pub fn push(&mut self, op: PaintOp) {
        self.ops.push(op);
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// Builds a display list from a laid-out scene.
pub fn paint(layout: &LayoutTree, clear: Color) -> DisplayList {
    let mut display = DisplayList::default();
    display.push(PaintOp::Clear(clear));

    for node in &layout.boxes {
        if node.style.background.a > 0 {
            display.push(PaintOp::FillRect {
                rect: node.rect,
                color: node.style.background,
            });
        }
        if node.style.border_width > 0.0 && node.style.border_color.a > 0 {
            display.push(PaintOp::StrokeRect {
                rect: node.rect,
                color: node.style.border_color,
                width: node.style.border_width,
            });
        }
        if let Some(text) = &node.text {
            if !text.is_empty() {
                display.push(PaintOp::DrawText {
                    origin: node.content.origin,
                    text: text.clone(),
                    font_size: node.style.font_size,
                    color: node.style.color,
                });
            }
        }
    }

    display
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;
    use crate::layout::LayoutBox;
    use crate::scene::NodeId;
    use crate::style::{Color, Style};

    #[test]
    fn paints_background_border_and_text() {
        let layout = LayoutTree {
            viewport: Size::new(100.0, 100.0),
            boxes: vec![LayoutBox {
                node_id: NodeId(1),
                parent: None,
                rect: Rect::new(0.0, 0.0, 100.0, 100.0),
                content: Rect::new(2.0, 2.0, 96.0, 96.0),
                style: Style::default()
                    .background(Color::RED)
                    .border(1.0, Color::WHITE)
                    .text(12.0, Color::BLACK),
                text: Some("hi".into()),
            }],
        };
        let display = paint(&layout, Color::WHITE);
        assert!(display
            .ops
            .iter()
            .any(|op| matches!(op, PaintOp::FillRect { .. })));
        assert!(display
            .ops
            .iter()
            .any(|op| matches!(op, PaintOp::StrokeRect { .. })));
        assert!(display
            .ops
            .iter()
            .any(|op| matches!(op, PaintOp::DrawText { .. })));
    }
}
