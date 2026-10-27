//! Versioned UiBatch encoding.
//! 渲染批可在进程内、IPC 或网络间复用；解码必须做边界检查。

use crate::geometry::{Point, Rect};
use crate::paint::PaintOp;
use crate::style::Color;

// BEGIN VERIFIED HEADER CONSTANTS TYPE
const MAGIC: [u8; 4] = [b'J', b'U', b'E', b'B'];
const VERSION: u16 = 1;
const MAX_OPS: u32 = 1_000_000;
// END VERIFIED HEADER CONSTANTS TYPE
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
                PaintOp::Line {
                    from,
                    to,
                    color,
                    width,
                } => {
                    out.push(7);
                    write_point(&mut out, *from);
                    write_point(&mut out, *to);
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
        let op_count = decode_header(bytes)?;
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
                7 => PaintOp::Line {
                    from: cursor.point()?,
                    to: cursor.point()?,
                    color: cursor.color()?,
                    width: cursor.f32()?,
                },
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

// BEGIN VERIFIED CURSOR READ KERNEL
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], BatchError> {
        match take_bytes(self.bytes, &mut self.pos, len) {
            Some(slice) => Ok(slice),
            None => Err(BatchError::Truncated),
        }
    }
}
// END VERIFIED CURSOR READ KERNEL

// BEGIN VERIFIED SCALAR READ KERNEL
impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Result<u8, BatchError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, BatchError> {
        let bytes = self.take(2)?;
        Ok(bytes[0] as u16 + (bytes[1] as u16) * 256)
    }

    fn u32(&mut self) -> Result<u32, BatchError> {
        let bytes = self.take(4)?;
        // Spell out little-endian weights so the production body, including
        // overflow safety, is verified without assuming a std conversion spec.
        // 显式写出小端字节权重，使生产函数及溢出安全直接可证，不额外假定标准库转换契约。
        Ok(bytes[0] as u32
            + (bytes[1] as u32) * 256
            + (bytes[2] as u32) * 65536
            + (bytes[3] as u32) * 16777216)
    }
}
// END VERIFIED SCALAR READ KERNEL

// BEGIN VERIFIED HEADER READ KERNEL
/// Validates the complete header before the decoder reserves operation storage.
/// 在解码器预分配操作存储之前校验完整帧头，保持原有错误优先顺序。
fn decode_header(bytes: &[u8]) -> Result<u32, BatchError> {
    if bytes.len() < 10 {
        return Err(BatchError::Truncated);
    }
    if bytes[0] != MAGIC[0] || bytes[1] != MAGIC[1] || bytes[2] != MAGIC[2] || bytes[3] != MAGIC[3]
    {
        return Err(BatchError::BadMagic);
    }
    let mut cursor = Cursor { bytes, pos: 4 };
    let version = cursor.u16()?;
    if version != VERSION {
        return Err(BatchError::UnsupportedVersion(version));
    }
    let op_count = cursor.u32()?;
    if op_count > MAX_OPS {
        return Err(BatchError::TooLarge);
    }
    // Every operation requires at least its tag; compare before allocation.
    // 每个操作至少需要标签字节；分配之前先比较可用载荷大小。
    if op_count as usize > bytes.len() - 10 {
        return Err(BatchError::Truncated);
    }
    Ok(op_count)
}
// END VERIFIED HEADER READ KERNEL

// BEGIN VERIFIED BOUNDED PAYLOAD KERNEL
impl<'a> Cursor<'a> {
    /// Reads a length-prefixed borrowed payload without allocating memory.
    /// 读取长度前缀限定的借用载荷，不分配内存；完整前缀被消费后，载荷错误不回退前缀。
    fn bounded_bytes(&mut self, max_len: u32) -> Result<&'a [u8], BatchError> {
        let len = self.u32()?;
        if len > max_len {
            return Err(BatchError::TooLarge);
        }
        self.take(len as usize)
    }
}
// END VERIFIED BOUNDED PAYLOAD KERNEL

impl<'a> Cursor<'a> {
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
        let bytes = self.bounded_bytes(MAX_STRING)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| BatchError::InvalidUtf8)
    }
}

// BEGIN VERIFIED DECODE RANGE KERNEL
/// Returns a legal read endpoint, rejecting invalid positions and lengths.
/// 返回合法读取终点；拒绝非法起点和长度，证明脚本直接提取原始函数体。
fn checked_read_end(total: usize, pos: usize, len: usize) -> Option<usize> {
    if pos > total {
        None
    } else if len > total - pos {
        None
    } else {
        Some(pos + len)
    }
}
// END VERIFIED DECODE RANGE KERNEL

// BEGIN VERIFIED BYTE READ KERNEL
/// Reads exactly len bytes, advancing only on success and preserving the input.
/// 精确读取 len 字节；只在成功时推进位置，失败保持位置且不修改输入。
fn take_bytes<'a>(bytes: &'a [u8], pos: &mut usize, len: usize) -> Option<&'a [u8]> {
    match checked_read_end(bytes.len(), *pos, len) {
        None => None,
        Some(end) => {
            let slice = &bytes[*pos..end];
            *pos = end;
            Some(slice)
        }
    }
}
// END VERIFIED BYTE READ KERNEL

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

// BEGIN VERIFIED BATCH ERROR TYPE
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
// END VERIFIED BATCH ERROR TYPE

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
            PaintOp::Line {
                from: Point::new(0.0, 1.0),
                to: Point::new(2.0, 3.0),
                color: Color::GREEN,
                width: 1.5,
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

    #[test]
    fn bounded_payload_returns_exact_bytes_and_keeps_following_data() {
        let bytes = [99, 3, 0, 0, 0, b'a', b'b', b'c', 42];
        let mut cursor = Cursor {
            bytes: &bytes,
            pos: 1,
        };
        assert_eq!(cursor.bounded_bytes(3).unwrap(), b"abc");
        assert_eq!(cursor.pos, 8);
        assert_eq!(cursor.u8(), Ok(42));
        let mut empty = Cursor {
            bytes: &[0; 4],
            pos: 0,
        };
        assert_eq!(empty.bounded_bytes(0).unwrap(), b"");
        assert_eq!(empty.pos, 4);
    }

    #[test]
    fn bounded_payload_distinguishes_prefix_limit_and_payload_failures() {
        let prefix = 3u32.to_le_bytes();
        for size in 0..4 {
            let mut cursor = Cursor {
                bytes: &prefix[..size],
                pos: 0,
            };
            assert_eq!(cursor.bounded_bytes(3), Err(BatchError::Truncated));
            assert_eq!(cursor.pos, 0);
        }
        let mut cursor = Cursor {
            bytes: &prefix,
            pos: 0,
        };
        assert_eq!(cursor.bounded_bytes(2), Err(BatchError::TooLarge));
        assert_eq!(cursor.pos, 4);
        cursor.pos = 0;
        assert_eq!(cursor.bounded_bytes(3), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, 4);
        let max_prefix = u32::MAX.to_le_bytes();
        let mut max = Cursor {
            bytes: &max_prefix,
            pos: 0,
        };
        assert_eq!(max.bounded_bytes(u32::MAX), Err(BatchError::Truncated));
        assert_eq!(max.pos, 4);
    }

    #[test]
    fn strings_keep_existing_utf8_and_length_error_states() {
        // Invalid UTF-8 is rejected after consuming its bounded payload.
        // 有界载荷读取完成后拒绝非法 UTF-8，保持既有游标状态，不伪报 UTF-8 已证明。
        let bytes = [2, 0, 0, 0, 0xc0, 0xaf];
        let mut cursor = Cursor {
            bytes: &bytes,
            pos: 0,
        };
        assert_eq!(cursor.string(), Err(BatchError::InvalidUtf8));
        assert_eq!(cursor.pos, 6);
        let oversized = (MAX_STRING + 1).to_le_bytes();
        let mut cursor = Cursor {
            bytes: &oversized,
            pos: 0,
        };
        assert_eq!(cursor.string(), Err(BatchError::TooLarge));
        assert_eq!(cursor.pos, 4);
    }

    #[test]
    fn u16_decoding_matches_native_conversion_for_entire_domain() {
        for value in 0..=u16::MAX {
            let bytes = value.to_le_bytes();
            let mut cursor = Cursor {
                bytes: &bytes,
                pos: 0,
            };
            assert_eq!(cursor.u16(), Ok(value));
            assert_eq!(cursor.pos, 2);
        }
        let mut cursor = Cursor {
            bytes: &[255],
            pos: 0,
        };
        assert_eq!(cursor.u16(), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, 0);
    }

    #[test]
    fn header_validation_preserves_error_precedence() {
        let mut bytes = UiBatch::default().encode();
        bytes[4..6].copy_from_slice(&2u16.to_le_bytes());
        bytes[6..10].copy_from_slice(&(MAX_OPS + 1).to_le_bytes());
        bytes[0] = b'X';
        assert_eq!(decode_header(&bytes[..9]), Err(BatchError::Truncated));
        assert_eq!(decode_header(&bytes), Err(BatchError::BadMagic));
        bytes[0] = b'J';
        assert_eq!(
            decode_header(&bytes),
            Err(BatchError::UnsupportedVersion(2))
        );
        bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
        assert_eq!(decode_header(&bytes), Err(BatchError::TooLarge));
        bytes[6..10].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(decode_header(&bytes), Err(BatchError::Truncated));
    }

    #[test]
    fn valid_header_does_not_claim_payload_is_valid() {
        let mut bytes = UiBatch::default().encode();
        assert_eq!(decode_header(&bytes), Ok(0));
        bytes.push(255);
        assert_eq!(decode_header(&bytes), Ok(0));
        assert_eq!(UiBatch::decode(&bytes), Err(BatchError::TrailingBytes));
        bytes[6..10].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(decode_header(&bytes), Ok(1));
        assert_eq!(UiBatch::decode(&bytes), Err(BatchError::UnknownOp(255)));
    }

    #[test]
    fn scalar_reads_match_native_endian_conversion_for_each_byte_lane() {
        for lane in 0..4 {
            for value in 0..=255u8 {
                let mut bytes = [0x12, 0x80, 0xfe, 0xff];
                bytes[lane] = value;
                let mut cursor = Cursor {
                    bytes: &bytes,
                    pos: 0,
                };
                assert_eq!(cursor.u32().unwrap(), u32::from_le_bytes(bytes));
                assert_eq!(cursor.pos, 4);
            }
        }
        for bytes in [[0; 4], [255; 4], [1, 2, 3, 4]] {
            let mut cursor = Cursor {
                bytes: &bytes,
                pos: 0,
            };
            assert_eq!(cursor.u32().unwrap(), u32::from_le_bytes(bytes));
        }
    }

    #[test]
    fn short_integer_read_preserves_position_and_can_be_followed_by_byte_reads() {
        let bytes = [9, 1, 2, 3];
        let mut cursor = Cursor {
            bytes: &bytes,
            pos: 1,
        };
        for end in 1..=4 {
            cursor.bytes = &bytes[..end];
            assert_eq!(cursor.u32(), Err(BatchError::Truncated));
            assert_eq!(cursor.pos, 1);
        }
        cursor.bytes = &bytes;
        assert_eq!(cursor.u8(), Ok(1));
        assert_eq!(cursor.u8(), Ok(2));
        assert_eq!(cursor.u8(), Ok(3));
        assert_eq!(cursor.u8(), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, 4);
    }

    #[test]
    fn byte_reads_preserve_all_possible_values() {
        let bytes: Vec<u8> = (0..=255).collect();
        let mut cursor = Cursor {
            bytes: &bytes,
            pos: 0,
        };
        for value in 0..=255u8 {
            assert_eq!(cursor.u8(), Ok(value));
        }
        assert_eq!(cursor.u8(), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, 256);
    }

    #[test]
    fn failed_reads_preserve_cursor_and_success_advances_exactly() {
        let mut cursor = Cursor {
            bytes: b"abc",
            pos: 1,
        };
        assert_eq!(cursor.take(usize::MAX), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, 1);
        assert_eq!(cursor.take(3), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, 1);
        assert_eq!(cursor.take(2).unwrap(), b"bc");
        assert_eq!(cursor.pos, 3);
        assert_eq!(cursor.take(0).unwrap(), b"");
        assert_eq!(cursor.pos, 3);
        assert_eq!(cursor.take(1), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, 3);
        cursor.pos = usize::MAX;
        assert_eq!(cursor.take(0), Err(BatchError::Truncated));
        assert_eq!(cursor.pos, usize::MAX);
    }

    #[test]
    fn every_incomplete_text_batch_prefix_is_rejected() {
        let bytes = UiBatch::new(vec![PaintOp::DrawText {
            origin: Point::ZERO,
            text: "中文🦀".into(),
            font_size: 16.0,
            color: Color::BLACK,
        }])
        .encode();
        for end in 0..bytes.len() {
            assert_eq!(
                UiBatch::decode(&bytes[..end]),
                Err(BatchError::Truncated),
                "prefix length {end}"
            );
        }
        assert!(UiBatch::decode(&bytes).is_ok());
    }

    #[test]
    fn header_cannot_reserve_more_ops_than_payload_tag_bytes() {
        let mut bytes = UiBatch::default().encode();
        bytes[6..10].copy_from_slice(&MAX_OPS.to_le_bytes());
        assert_eq!(UiBatch::decode(&bytes), Err(BatchError::Truncated));
        bytes[6..10].copy_from_slice(&2u32.to_le_bytes());
        bytes.push(6);
        assert_eq!(UiBatch::decode(&bytes), Err(BatchError::Truncated));
        bytes[6..10].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(UiBatch::decode(&bytes).unwrap().ops, vec![PaintOp::PopClip]);
    }
}
