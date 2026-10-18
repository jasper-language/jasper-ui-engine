//! Versioned UiBatch encoding.
//! 渲染批可在进程内、IPC 或网络间复用；解码必须做边界检查。

use crate::geometry::{Point, Rect};
use crate::paint::PaintOp;
use crate::style::Color;

const MAGIC: [u8; 4] = *b"JUEB";
const VERSION: u16 = 1;
const MAX_OPS: u32 = 1_000_000;
const MAX_STRING: u32 = 16 * 1024 * 1024;

/// A batch of paint operations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiBatch {
    pub ops: Vec<PaintOp>,
}

impl UiBatch {
    pub fn new(ops: Vec<PaintOp>) -> Self {
        Self { ops }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + self.ops.len() * 32);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(self.ops.len() as u32).to_le_bytes());
        for op in &self.ops {
            match op {
                PaintOp::Clear(color) => {
                    out.push(1);
                    write_color(&mut out, *color);
                }
                PaintOp::FillRect { rect, color } => {
                    out.push(2);
                    write_rect(&mut out, *rect);
                    write_color(&mut out, *color);
                }
                PaintOp::StrokeRect { rect, color, width } => {
                    out.push(3);
                    write_rect(&mut out, *rect);
                    write_color(&mut out, *color);
                    out.extend_from_slice(&width.to_le_bytes());
                }
                PaintOp::DrawText {
                    origin,
                    text,
                    font_size,
                    color,
                } => {
                    out.push(4);
                    write_point(&mut out, *origin);
                    write_string(&mut out, text);
                    out.extend_from_slice(&font_size.to_le_bytes());
                    write_color(&mut out, *color);
                }
                PaintOp::PushClip(rect) => {
                    out.push(5);
                    write_rect(&mut out, *rect);
                }
                PaintOp::PopClip => out.push(6),
            }
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, BatchError> {
        if bytes.len() < 10 {
            return Err(BatchError::Truncated);
        }
        if bytes[0..4] != MAGIC {
            return Err(BatchError::BadMagic);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION {
            return Err(BatchError::UnsupportedVersion(version));
        }
        let op_count = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
        if op_count > MAX_OPS {
            return Err(BatchError::TooLarge);
        }
        let mut cursor = Cursor { bytes, pos: 10 };
        let mut ops = Vec::with_capacity(op_count as usize);
        for _ in 0..op_count {
            let tag = cursor.u8()?;
            let op = match tag {
                1 => PaintOp::Clear(cursor.color()?),
                2 => PaintOp::FillRect {
                    rect: cursor.rect()?,
                    color: cursor.color()?,
                },
                3 => PaintOp::StrokeRect {
                    rect: cursor.rect()?,
                    color: cursor.color()?,
                    width: cursor.f32()?,
                },
                4 => PaintOp::DrawText {
                    origin: cursor.point()?,
                    text: cursor.string()?,
                    font_size: cursor.f32()?,
                    color: cursor.color()?,
                },
                5 => PaintOp::PushClip(cursor.rect()?),
                6 => PaintOp::PopClip,
                _ => return Err(BatchError::UnknownOp(tag)),
            };
            ops.push(op);
        }
        if cursor.pos != bytes.len() {
            return Err(BatchError::TrailingBytes);
        }
        Ok(Self { ops })
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], BatchError> {
        let end = self.pos.checked_add(len).ok_or(BatchError::Truncated)?;
        if end > self.bytes.len() {
            return Err(BatchError::Truncated);
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, BatchError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, BatchError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn f32(&mut self) -> Result<f32, BatchError> {
        let bytes = self.take(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn color(&mut self) -> Result<Color, BatchError> {
        let bytes = self.take(4)?;
        Ok(Color::new(bytes[0], bytes[1], bytes[2], bytes[3]))
    }

    fn point(&mut self) -> Result<Point, BatchError> {
        Ok(Point::new(self.f32()?, self.f32()?))
    }

    fn rect(&mut self) -> Result<Rect, BatchError> {
        Ok(Rect::new(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }

    fn string(&mut self) -> Result<String, BatchError> {
        let len = self.u32()?;
        if len > MAX_STRING {
            return Err(BatchError::TooLarge);
        }
        let bytes = self.take(len as usize)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| BatchError::InvalidUtf8)
    }
}

fn write_color(out: &mut Vec<u8>, color: Color) {
    out.extend_from_slice(&[color.r, color.g, color.b, color.a]);
}

fn write_point(out: &mut Vec<u8>, point: Point) {
    out.extend_from_slice(&point.x.to_le_bytes());
    out.extend_from_slice(&point.y.to_le_bytes());
}

fn write_rect(out: &mut Vec<u8>, rect: Rect) {
    write_point(out, rect.origin);
    out.extend_from_slice(&rect.size.width.to_le_bytes());
    out.extend_from_slice(&rect.size.height.to_le_bytes());
}

fn write_string(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// Batch decoding failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchError {
    BadMagic,
    UnsupportedVersion(u16),
    Truncated,
    TooLarge,
    InvalidUtf8,
    UnknownOp(u8),
    TrailingBytes,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Point, Rect};
    use crate::style::Color;

    #[test]
    fn round_trips_all_ops() {
        let batch = UiBatch::new(vec![
            PaintOp::Clear(Color::WHITE),
            PaintOp::FillRect {
                rect: Rect::new(1.0, 2.0, 3.0, 4.0),
                color: Color::RED,
            },
            PaintOp::StrokeRect {
                rect: Rect::new(5.0, 6.0, 7.0, 8.0),
                color: Color::BLUE,
                width: 2.0,
            },
            PaintOp::DrawText {
                origin: Point::new(9.0, 10.0),
                text: "你好".into(),
                font_size: 12.0,
                color: Color::BLACK,
            },
            PaintOp::PushClip(Rect::new(0.0, 0.0, 20.0, 20.0)),
            PaintOp::PopClip,
        ]);
        let bytes = batch.encode();
        assert_eq!(UiBatch::decode(&bytes).unwrap(), batch);
    }

    #[test]
    fn rejects_bad_magic_and_truncation() {
        assert_eq!(
            UiBatch::decode(b"NOPE\x01\x00\x00\x00\x00\x00"),
            Err(BatchError::BadMagic)
        );
        assert_eq!(
            UiBatch::decode(b"JUEB\x01\x00\x05\x00\x00"),
            Err(BatchError::Truncated)
        );
    }
}
