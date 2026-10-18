//! Text measurement seam.
//! 文本测量是独立子系统；真实 Parley 适配在后续版本替换这里的近似实现。

/// Measured text box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub lines: u32,
}

/// Measurement contract used by layout.
pub trait TextEngine {
    /// Measures `text` at `font_size`, optionally wrapping at `max_width`.
    fn measure(&self, text: &str, font_size: f32, max_width: Option<f32>) -> TextMetrics;
}

/// Reference implementation with deterministic but approximate metrics.
///
/// It is intentionally simple: Latin glyphs are ~0.6em wide, CJK/full-width
/// glyphs ~1.0em. It is not a shaping engine, does not handle kerning, emoji,
/// bidi or font fallback; Parley replaces it before UI text is considered done.
#[derive(Clone, Copy, Debug, Default)]
pub struct SimpleTextEngine;

impl TextEngine for SimpleTextEngine {
    fn measure(&self, text: &str, font_size: f32, max_width: Option<f32>) -> TextMetrics {
        let font_size = font_size.max(1.0);
        let line_height = font_size * 1.25;
        let mut lines = 0u32;
        let mut max_line = 0.0f32;

        for raw_line in text.split('\n') {
            let mut line_width = 0.0f32;
            let mut count = 0u32;
            for ch in raw_line.chars() {
                let advance = if is_wide(ch) {
                    font_size
                } else {
                    font_size * 0.6
                };
                if let Some(limit) = max_width {
                    if line_width + advance > limit && line_width > 0.0 {
                        max_line = max_line.max(line_width);
                        lines += 1;
                        line_width = 0.0;
                    }
                }
                line_width += advance;
                count += 1;
            }
            if count == 0 {
                lines += 1;
            } else {
                max_line = max_line.max(line_width);
                lines += 1;
            }
        }

        let width = max_width.map(|w| max_line.min(w)).unwrap_or(max_line);
        TextMetrics {
            width,
            height: line_height * lines as f32,
            ascent: font_size,
            lines,
        }
    }
}

fn is_wide(ch: char) -> bool {
    matches!(ch as u32,
        0x1100..=0x115F
        | 0x2E80..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_latin_and_cjk() {
        let engine = SimpleTextEngine;
        let latin = engine.measure("abc", 10.0, None);
        assert!((latin.width - 18.0).abs() < 0.01);
        let cjk = engine.measure("你好", 10.0, None);
        assert!((cjk.width - 20.0).abs() < 0.01);
    }

    #[test]
    fn wraps_at_max_width() {
        let engine = SimpleTextEngine;
        let metrics = engine.measure("abcdef", 10.0, Some(18.0));
        assert_eq!(metrics.lines, 2);
        assert!(metrics.height > 20.0);
    }
}
