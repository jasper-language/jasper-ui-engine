//! Render backend contract and the headless software backend.
//! GPU/window backends implement the same trait; the engine never depends on Vello types.

use std::io;
use std::path::Path;

use crate::batch::UiBatch;
use crate::geometry::Size;
use crate::paint::PaintOp;
use crate::raster::Canvas;

/// A backend receives versioned batches and presents frames.
// BEGIN VERIFIED NATIVE BACKEND TRAIT CONTRACT
pub trait RenderBackend {
    fn resize(&mut self, size: Size);
    fn submit(&mut self, batch: &UiBatch) -> Result<(), BackendError>;
    /// Consume a completed packed RGBA8 native frame; no legacy paint replay.
    /// 接收已绘制的紧密 RGBA8 原生帧，不重放旧绘制算法；不支持时明确拒绝。
    fn submit_rgba8(&mut self, _width:u32, _height:u32, _pixels:Vec<u8>) -> Result<(),BackendError> {
        Err(BackendError::Unsupported("native RGBA8 frame submission"))
    }
    fn present(&mut self) -> Result<(), BackendError>;
}
// END VERIFIED NATIVE BACKEND TRAIT CONTRACT

/// Backend failures that callers can handle.
// BEGIN VERIFIED NATIVE BACKEND ERROR TYPE
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    Unsupported(&'static str),
    Invalid(String),
    Io(String),
}
// END VERIFIED NATIVE BACKEND ERROR TYPE

impl From<io::Error> for BackendError {
    fn from(error: io::Error) -> Self {
        BackendError::Io(error.to_string())
    }
}

// BEGIN VERIFIED NATIVE BACKEND SUBMISSION KERNEL
/// Submit one completed native frame and present it only after successful adoption.
pub(crate) fn submit_native_frame(
    backend: &mut impl RenderBackend,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) -> Result<(), BackendError> {
    backend.submit_rgba8(width, height, pixels)?;
    backend.present()
}
// END VERIFIED NATIVE BACKEND SUBMISSION KERNEL

/// CPU backend for tests, CI and GPU-less hosts.
// BEGIN VERIFIED NATIVE HEADLESS BACKEND TYPE
#[derive(Clone, Debug)]
pub struct HeadlessBackend {
    canvas: Canvas,
}
// END VERIFIED NATIVE HEADLESS BACKEND TYPE

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

// BEGIN VERIFIED NATIVE HEADLESS BACKEND FORWARDING KERNEL
impl HeadlessBackend {
    fn submit_native_pixels(
        &mut self,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Result<(), BackendError> {
        self.import_native_frame(width, height, pixels)
    }
}
// END VERIFIED NATIVE HEADLESS BACKEND FORWARDING KERNEL

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

    /// Adopt native pixels through the existing checked storage import, with no redraw algorithm.
    /// 经既有已检查的存储导入接管原生像素；不执行旧光栅绘制，拒绝时保留前帧。
    fn submit_rgba8(&mut self,width:u32,height:u32,pixels:Vec<u8>)->Result<(),BackendError> {
        self.submit_native_pixels(width,height,pixels)
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

// BEGIN VERIFIED NATIVE BACKEND FRAME IMPORT KERNEL
impl HeadlessBackend {
    fn import_native_frame(&mut self,width:u32,height:u32,pixels:Vec<u8>)->Result<(),BackendError> {
        match Canvas::from_rgba8(width,height,pixels) {
            Ok(canvas)=>{self.canvas=canvas;Ok(())},
            Err(_)=>Err(BackendError::Invalid("invalid packed RGBA8 frame".into())),
        }
    }
}
// END VERIFIED NATIVE BACKEND FRAME IMPORT KERNEL
