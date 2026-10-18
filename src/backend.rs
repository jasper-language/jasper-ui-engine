//! Render backend contract and the headless software backend.
//! GPU/window backends implement the same trait; the engine never depends on Vello types.

use std::io;
use std::path::Path;

use crate::batch::UiBatch;
use crate::geometry::Size;
use crate::paint::PaintOp;
use crate::raster::Canvas;

/// A backend receives versioned batches and presents frames.
pub trait RenderBackend {
    fn resize(&mut self, size: Size);
    fn submit(&mut self, batch: &UiBatch) -> Result<(), BackendError>;
    fn present(&mut self) -> Result<(), BackendError>;
}

/// Backend failures that callers can handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    Unsupported(&'static str),
    Invalid(String),
    Io(String),
}

impl From<io::Error> for BackendError {
    fn from(error: io::Error) -> Self {
        BackendError::Io(error.to_string())
    }
}

/// CPU backend for tests, CI and GPU-less hosts.
#[derive(Clone, Debug)]
pub struct HeadlessBackend {
    canvas: Canvas,
}

impl HeadlessBackend {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            canvas: Canvas::new(width, height),
        }
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn canvas_mut(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    pub fn save_ppm(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.canvas.save_ppm(path)
    }
}

impl RenderBackend for HeadlessBackend {
    fn resize(&mut self, size: Size) {
        let width = size.width.max(1.0).round() as u32;
        let height = size.height.max(1.0).round() as u32;
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas = Canvas::new(width, height);
        }
    }

    fn submit(&mut self, batch: &UiBatch) -> Result<(), BackendError> {
        for op in &batch.ops {
            match op {
                PaintOp::Clear(color) => self.canvas.clear(*color),
                PaintOp::FillRect { rect, color } => self.canvas.fill_rect(*rect, *color),
                PaintOp::StrokeRect { rect, color, width } => {
                    self.canvas.stroke_rect(*rect, *color, *width)
                }
                PaintOp::Line {
                    from,
                    to,
                    color,
                    width,
                } => self.canvas.line(*from, *to, *color, *width),
                PaintOp::PushClip(rect) => self.canvas.push_clip(*rect),
                PaintOp::PopClip => self.canvas.pop_clip(),
                PaintOp::DrawText { .. } => {
                    return Err(BackendError::Unsupported(
                        "text rasterization is not implemented",
                    ));
                }
            }
        }
        Ok(())
    }

    fn present(&mut self) -> Result<(), BackendError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::paint::PaintOp;
    use crate::style::Color;

    #[test]
    fn clears_and_fills() {
        let mut backend = HeadlessBackend::new(4, 4);
        let batch = UiBatch::new(vec![
            PaintOp::Clear(Color::WHITE),
            PaintOp::FillRect {
                rect: Rect::new(0.0, 0.0, 2.0, 2.0),
                color: Color::RED,
            },
        ]);
        backend.resize(Size::new(4.0, 4.0));
        backend.submit(&batch).unwrap();
        backend.present().unwrap();
        assert_eq!(backend.canvas().pixel(0, 0).unwrap(), Color::RED);
        assert_eq!(backend.canvas().pixel(3, 3).unwrap(), Color::WHITE);
    }

    #[test]
    fn text_is_explicitly_unsupported() {
        let mut backend = HeadlessBackend::new(4, 4);
        let batch = UiBatch::new(vec![PaintOp::DrawText {
            origin: crate::geometry::Point::ZERO,
            text: "x".into(),
            font_size: 10.0,
            color: Color::BLACK,
        }]);
        assert_eq!(
            backend.submit(&batch),
            Err(BackendError::Unsupported(
                "text rasterization is not implemented"
            ))
        );
    }
}
