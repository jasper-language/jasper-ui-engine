//! Pending explicit-font text preparation. Not accepted for production use.
//! The prepared third-party layout is shared by future CPU/GPU adapters.
use parley::{Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, StyleProperty};
use parley::fontique::{Blob, GenericFamily};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    MissingFonts,
    InvalidFont,
    InvalidGeometry,
    TextBudgetExceeded,
}

/// Single-owner preparation context; no system font discovery is enabled.
/// Font parsing/shaping is an explicitly unproved third-party trust boundary.
pub struct ParleyTextPreparer {
    fonts: FontContext,
    layouts: LayoutContext<[u8; 4]>,
    max_text_bytes: usize,
}

impl ParleyTextPreparer {
    /// Registers only caller-owned bytes. The font count/byte budget is host policy.
    pub fn new(fonts: Vec<Vec<u8>>, max_text_bytes: usize) -> Result<Self, PrepareError> {
        if fonts.is_empty() {
            return Err(PrepareError::MissingFonts);
        }
        let mut context = FontContext::new();
        let mut families = Vec::new();
        for bytes in fonts {
            let registered = context.collection.register_fonts(Blob::new(Arc::new(bytes)), None);
            if registered.is_empty() {
                return Err(PrepareError::InvalidFont);
            }
            families.extend(registered.into_iter().map(|(id, _)| id));
        }
        context.collection.set_generic_families(GenericFamily::SansSerif, families.into_iter());
        Ok(Self { fonts: context, layouts: LayoutContext::new(), max_text_bytes })
    }

    /// Prepares once at final width; no document mutation or input consumption.
    /// Missing glyph coverage remains a reported third-party limitation, not a fallback font grant.
    pub fn prepare(
        &mut self,
        text: &str,
        font_size: f32,
        max_width: Option<f32>,
        rgba: [u8; 4],
    ) -> Result<PreparedText, PrepareError> {
        if !font_size.is_finite() || font_size <= 0.0
            || max_width.is_some_and(|width| !width.is_finite() || width <= 0.0)
        {
            return Err(PrepareError::InvalidGeometry);
        }
        if text.len() > self.max_text_bytes {
            return Err(PrepareError::TextBudgetExceeded);
        }
        let mut builder = self.layouts.ranged_builder(&mut self.fonts, text, 1.0, false);
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::Brush(rgba));
        let mut layout = builder.build(text);
        layout.break_all_lines(max_width);
        layout.align(Alignment::Start, AlignmentOptions::default());
        if !layout.width().is_finite() || !layout.height().is_finite() {
            return Err(PrepareError::InvalidGeometry);
        }
        Ok(PreparedText { text: text.to_owned(), layout })
    }
}

/// Immutable text/layout pair: hit results cannot be checked against unrelated text.
/// Pending adapter API, not a frozen Jasper public text protocol.
pub struct PreparedText {
    text: String,
    layout: Layout<[u8; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaretAffinity { Upstream, Downstream }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextHit {
    pub byte_index: usize,
    pub affinity: CaretAffinity,
}

impl PreparedText {
    pub fn text(&self) -> &str { &self.text }

    /// Adapter-only immutable access for CPU/GPU consumption of the same layout.
    pub fn layout(&self) -> &Layout<[u8; 4]> { &self.layout }

    /// Coordinates are local to this layout; scene/scroll mapping belongs to UI.
    /// Returns a UTF-8 byte boundary, never an edit authorization or document offset.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<TextHit> {
        if !x.is_finite() || !y.is_finite() { return None; }
        let cursor = parley::editing::Cursor::from_point(&self.layout, x, y);
        let index = cursor.index();
        if !self.text.is_char_boundary(index) { return None; }
        let affinity = match cursor.affinity() {
            parley::Affinity::Upstream => CaretAffinity::Upstream,
            parley::Affinity::Downstream => CaretAffinity::Downstream,
        };
        Some(TextHit { byte_index: index, affinity })
    }
}
