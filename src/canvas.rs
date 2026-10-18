//! Custom-drawing canvas component.
//! 宿主中立的 Canvas：记录局部坐标下的绘制指令，由布局/绘制阶段嵌入场景。
//!
//! 这不是浏览器的 `<canvas>` 后端，也不做位图存储：它只保存**可序列化的绘制指令**，
//! 因此同一份 Canvas 可以在软件光栅、GPU、Web Canvas 2D 或远程客户端上重放。

use crate::geometry::{Point, Rect};
use crate::paint::PaintOp;
use crate::style::Color;

/// One drawing command in canvas-local coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasOp {
    FillRect {
        rect: Rect,
        color: Color,
    },
    StrokeRect {
        rect: Rect,
        color: Color,
        width: f32,
    },
    Line {
        from: Point,
        to: Point,
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

/// A recorded canvas. Coordinates are relative to the canvas content box.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    ops: Vec<CanvasOp>,
}

impl Canvas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_ops(ops: Vec<CanvasOp>) -> Self {
        Self { ops }
    }

    pub fn ops(&self) -> &[CanvasOp] {
        &self.ops
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) -> &mut Self {
        self.ops.push(CanvasOp::FillRect { rect, color });
        self
    }

    pub fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32) -> &mut Self {
        self.ops.push(CanvasOp::StrokeRect { rect, color, width });
        self
    }

    pub fn line(&mut self, from: Point, to: Point, color: Color, width: f32) -> &mut Self {
        self.ops.push(CanvasOp::Line {
            from,
            to,
            color,
            width,
        });
        self
    }

    pub fn text(
        &mut self,
        origin: Point,
        text: impl Into<String>,
        font_size: f32,
        color: Color,
    ) -> &mut Self {
        self.ops.push(CanvasOp::DrawText {
            origin,
            text: text.into(),
            font_size,
            color,
        });
        self
    }

    pub fn push_clip(&mut self, rect: Rect) -> &mut Self {
        self.ops.push(CanvasOp::PushClip(rect));
        self
    }

    pub fn pop_clip(&mut self) -> &mut Self {
        self.ops.push(CanvasOp::PopClip);
        self
    }

    /// Converts local commands to absolute paint operations.
    pub fn to_paint_ops(&self, offset: Point) -> Vec<PaintOp> {
        self.ops
            .iter()
            .map(|op| match op {
                CanvasOp::FillRect { rect, color } => PaintOp::FillRect {
                    rect: offset_rect(*rect, offset),
                    color: *color,
                },
                CanvasOp::StrokeRect { rect, color, width } => PaintOp::StrokeRect {
                    rect: offset_rect(*rect, offset),
                    color: *color,
                    width: *width,
                },
                CanvasOp::Line {
                    from,
                    to,
                    color,
                    width,
                } => PaintOp::Line {
                    from: offset_point(*from, offset),
                    to: offset_point(*to, offset),
                    color: *color,
                    width: *width,
                },
                CanvasOp::DrawText {
                    origin,
                    text,
                    font_size,
                    color,
                } => PaintOp::DrawText {
                    origin: offset_point(*origin, offset),
                    text: text.clone(),
                    font_size: *font_size,
                    color: *color,
                },
                CanvasOp::PushClip(rect) => PaintOp::PushClip(offset_rect(*rect, offset)),
                CanvasOp::PopClip => PaintOp::PopClip,
            })
            .collect()
    }
}

fn offset_point(point: Point, offset: Point) -> Point {
    Point::new(point.x + offset.x, point.y + offset.y)
}

fn offset_rect(rect: Rect, offset: Point) -> Rect {
    Rect::new(
        rect.x() + offset.x,
        rect.y() + offset.y,
        rect.width(),
        rect.height(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn records_and_offsets_commands() {
        let mut canvas = Canvas::new();
        canvas
            .fill_rect(Rect::new(1.0, 2.0, 3.0, 4.0), Color::RED)
            .line(
                Point::new(0.0, 0.0),
                Point::new(5.0, 5.0),
                Color::BLACK,
                2.0,
            );
        let ops = canvas.to_paint_ops(Point::new(10.0, 20.0));
        assert_eq!(
            ops[0],
            PaintOp::FillRect {
                rect: Rect::new(11.0, 22.0, 3.0, 4.0),
                color: Color::RED
            }
        );
        assert_eq!(
            ops[1],
            PaintOp::Line {
                from: Point::new(10.0, 20.0),
                to: Point::new(15.0, 25.0),
                color: Color::BLACK,
                width: 2.0
            }
        );
    }
}
