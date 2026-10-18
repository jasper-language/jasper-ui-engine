//! Real text measurement backed by cosmic-text.
//! 使用系统字体、真实 shaping、字体回退、双向文本与断行；不改变 `TextEngine` 契约。

use std::cell::RefCell;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};

use crate::text::{TextEngine, TextMetrics};

/// Thread-local (not `Sync`) text engine owning one font system.
pub struct CosmicTextEngine {
    font_system: RefCell<FontSystem>,
}

impl Default for CosmicTextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CosmicTextEngine {
    pub fn new() -> Self {
        Self {
            font_system: RefCell::new(FontSystem::new()),
        }
    }
}

impl TextEngine for CosmicTextEngine {
    fn measure(&self, text: &str, font_size: f32, max_width: Option<f32>) -> TextMetrics {
        let font_size = font_size.max(1.0);
        let line_height = (font_size * 1.25).max(1.0);
        let mut font_system = self.font_system.borrow_mut();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(font_size, line_height));
        buffer.set_size(max_width.map(|width| width.max(0.0)), None);
        buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);

        let mut width = 0.0f32;
        let mut lines = 0u32;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            lines += 1;
        }
        let lines = lines.max(1);
        TextMetrics {
            width: max_width
                .map(|limit| width.min(limit.max(0.0)))
                .unwrap_or(width),
            height: line_height * lines as f32,
            ascent: font_size,
            lines,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_shaped_text_and_wraps_with_real_fonts() {
        let engine = CosmicTextEngine::new();

        let empty = engine.measure("", 16.0, None);
        assert_eq!(empty.lines, 1);
        assert_eq!(empty.width, 0.0);

        let latin = engine.measure("Hello", 16.0, None);
        assert_eq!(latin.lines, 1);
        assert!(latin.width > 0.0);

        let cjk = engine.measure("你好", 16.0, None);
        assert_eq!(cjk.lines, 1);
        assert!(cjk.width > 0.0);

        let wrapped = engine.measure("hello world hello world", 16.0, Some(60.0));
        assert!(wrapped.lines >= 2);
        assert!(wrapped.width <= 60.0);
    }
}
