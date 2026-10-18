//! tiny-skia CPU renderer for the recorded [`Canvas`] component.
//! 只做光栅化；不参与布局、输入或场景协议。

use tiny_skia::{
    FillRule, LineCap, LineJoin, Mask, Paint, PathBuilder, Pixmap, Rect as SkRect, Stroke,
    Transform,
};

use crate::canvas::{Canvas, CanvasOp};
use crate::geometry::{Point, Rect};
use crate::style::Color;

/// Result of rendering one recorded canvas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanvasRenderReport {
    pub drawn: usize,
    /// Text needs a shaper (Parley/cosmic-text); the rasterizer counts and skips it.
    pub skipped_text: usize,
}

/// Renders a canvas into a new premultiplied RGBA pixmap.
///
/// `origin` is added to every command, allowing the caller to blit a canvas
/// into a larger surface without rewriting the recorded display list.
pub fn render_canvas(
    canvas: &Canvas,
    pixmap_width: u32,
    pixmap_height: u32,
    origin: Point,
) -> Option<(Pixmap, CanvasRenderReport)> {
    let mut pixmap = Pixmap::new(pixmap_width, pixmap_height)?;
    let mut clips: Vec<Rect> = Vec::new();
    let mut report = CanvasRenderReport::default();

    for op in canvas.ops() {
        match op {
            CanvasOp::PushClip(rect) => clips.push(*rect),
            CanvasOp::PopClip => {
                clips.pop();
            }
            CanvasOp::FillRect { rect, color } => {
                let rect = sk_rect(offset_rect(*rect, origin))?;
                let mask = clip_mask(&clips, pixmap_width, pixmap_height);
                pixmap.fill_rect(rect, &paint(*color), Transform::identity(), mask.as_ref());
                report.drawn += 1;
            }
            CanvasOp::StrokeRect { rect, color, width } => {
                let path = PathBuilder::from_rect(sk_rect(offset_rect(*rect, origin))?);
                let mask = clip_mask(&clips, pixmap_width, pixmap_height);
                pixmap.stroke_path(
                    &path,
                    &paint(*color),
                    &stroke(*width),
                    Transform::identity(),
                    mask.as_ref(),
                );
                report.drawn += 1;
            }
            CanvasOp::Line {
                from,
                to,
                color,
                width,
            } => {
                let from = offset_point(*from, origin);
                let to = offset_point(*to, origin);
                let mut builder = PathBuilder::new();
                builder.move_to(from.x, from.y);
                builder.line_to(to.x, to.y);
                let Some(path) = builder.finish() else {
                    continue;
                };
                let mask = clip_mask(&clips, pixmap_width, pixmap_height);
                pixmap.stroke_path(
                    &path,
                    &paint(*color),
                    &stroke(*width),
                    Transform::identity(),
                    mask.as_ref(),
                );
                report.drawn += 1;
            }
            CanvasOp::DrawText { .. } => {
                report.skipped_text += 1;
            }
        }
    }

    Some((pixmap, report))
}

fn paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;
    paint
}

fn stroke(width: f32) -> Stroke {
    Stroke {
        width: width.max(0.01),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

fn clip_mask(clips: &[Rect], width: u32, height: u32) -> Option<Mask> {
    let first = *clips.first()?;
    let mut clip = first;
    for rect in &clips[1..] {
        let x0 = clip.x().max(rect.x());
        let y0 = clip.y().max(rect.y());
        let x1 = clip.right().min(rect.right());
        let y1 = clip.bottom().min(rect.bottom());
        clip = Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0));
    }
    let mut mask = Mask::new(width, height)?;
    mask.fill_path(
        &PathBuilder::from_rect(sk_rect(clip)?),
        FillRule::Winding,
        true,
        Transform::identity(),
    );
    Some(mask)
}

fn sk_rect(rect: Rect) -> Option<SkRect> {
    SkRect::from_xywh(rect.x(), rect.y(), rect.width(), rect.height())
}

fn offset_rect(rect: Rect, offset: Point) -> Rect {
    Rect::new(
        rect.x() + offset.x,
        rect.y() + offset.y,
        rect.width(),
        rect.height(),
    )
}

fn offset_point(point: Point, offset: Point) -> Point {
    Point::new(point.x + offset.x, point.y + offset.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn renders_shapes_and_honours_clip() {
        let mut canvas = Canvas::new();
        canvas
            .fill_rect(Rect::new(0.0, 0.0, 4.0, 4.0), Color::new(255, 0, 0, 255))
            .push_clip(Rect::new(0.0, 0.0, 2.0, 2.0))
            .fill_rect(Rect::new(0.0, 0.0, 4.0, 4.0), Color::new(0, 0, 255, 255))
            .pop_clip()
            .line(
                Point::new(0.0, 6.5),
                Point::new(6.0, 6.5),
                Color::new(0, 255, 0, 255),
                1.0,
            );

        let (pixmap, report) = render_canvas(&canvas, 8, 8, Point::ZERO).unwrap();
        assert_eq!(report.drawn, 3);
        assert_eq!(pixmap.pixel(1, 1).unwrap().blue(), 255);
        assert_eq!(pixmap.pixel(3, 3).unwrap().red(), 255);
        assert_eq!(pixmap.pixel(3, 6).unwrap().green(), 255);
    }

    #[test]
    fn counts_text_without_a_shaper() {
        let mut canvas = Canvas::new();
        canvas.text(Point::ZERO, "hi", 12.0, Color::BLACK);
        let (_, report) = render_canvas(&canvas, 4, 4, Point::ZERO).unwrap();
        assert_eq!(report.skipped_text, 1);
    }
}
