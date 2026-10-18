//! Portable style subset (PSS) used by the engine.
//! 可移植样式子集；不是完整 CSS，公共语义由 Jasper UI 定义，这里只承载引擎需要的部分。

use crate::geometry::Edges;

/// A CSS-like length in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
}

impl Length {
    /// Resolves against a parent size. `None` for `Auto`.
    pub fn resolve(self, parent: f32) -> Option<f32> {
        match self {
            Length::Auto => None,
            Length::Px(value) => Some(value.max(0.0)),
            Length::Percent(value) => Some((parent * value / 100.0).max(0.0)),
        }
    }
}

/// Layout mode for a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    Block,
    #[default]
    Flex,
    None,
}

/// Main-axis direction for flex layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    #[default]
    Column,
}

/// Cross-axis alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// Overflow behavior for children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
}

/// Main-axis distribution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// RGBA color; alpha is straight (not premultiplied).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const RED: Self = Self {
        r: 220,
        g: 50,
        b: 47,
        a: 255,
    };
    pub const GREEN: Self = Self {
        r: 46,
        g: 160,
        b: 67,
        a: 255,
    };
    pub const BLUE: Self = Self {
        r: 30,
        g: 110,
        b: 220,
        a: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Source-over compositing: `self` over `dst`.
    pub fn blend_over(self, dst: Color) -> Color {
        if self.a == 0 {
            return dst;
        }
        if self.a == 255 {
            return self;
        }
        let sa = self.a as f32 / 255.0;
        let da = dst.a as f32 / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a <= f32::EPSILON {
            return Color::TRANSPARENT;
        }
        let blend = |s: u8, d: u8| -> u8 {
            let s = s as f32;
            let d = d as f32;
            ((s * sa + d * da * (1.0 - sa)) / out_a)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Color {
            r: blend(self.r, dst.r),
            g: blend(self.g, dst.g),
            b: blend(self.b, dst.b),
            a: (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::TRANSPARENT
    }
}

/// Portable style for one node.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub display: Display,
    pub direction: FlexDirection,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub max_width: Length,
    pub min_height: Length,
    pub max_height: Length,
    pub padding: Edges,
    pub margin: Edges,
    pub gap: f32,
    pub align_items: Align,
    pub justify_content: Justify,
    pub flex_grow: f32,
    pub background: Color,
    pub border_width: f32,
    pub border_color: Color,
    pub font_size: f32,
    pub color: Color,
    pub overflow: Overflow,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            direction: FlexDirection::Column,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            max_width: Length::Auto,
            min_height: Length::Auto,
            max_height: Length::Auto,
            padding: Edges::ZERO,
            margin: Edges::ZERO,
            gap: 0.0,
            align_items: Align::Stretch,
            justify_content: Justify::Start,
            flex_grow: 0.0,
            background: Color::TRANSPARENT,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            font_size: 14.0,
            color: Color::BLACK,
            overflow: Overflow::Visible,
        }
    }
}

impl Style {
    pub fn size(mut self, width: Length, height: Length) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    pub fn margin(mut self, margin: Edges) -> Self {
        self.margin = margin;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn direction(mut self, direction: FlexDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn display(mut self, display: Display) -> Self {
        self.display = display;
        self
    }

    pub fn align_items(mut self, align: Align) -> Self {
        self.align_items = align;
        self
    }

    pub fn justify_content(mut self, justify: Justify) -> Self {
        self.justify_content = justify;
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow.max(0.0);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border_width = width.max(0.0);
        self.border_color = color;
        self
    }

    pub fn text(mut self, font_size: f32, color: Color) -> Self {
        self.font_size = font_size.max(1.0);
        self.color = color;
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_lengths() {
        assert_eq!(Length::Px(10.0).resolve(100.0), Some(10.0));
        assert_eq!(Length::Percent(50.0).resolve(200.0), Some(100.0));
        assert_eq!(Length::Auto.resolve(100.0), None);
    }

    #[test]
    fn blends_alpha() {
        let dst = Color::rgb(0, 0, 0);
        let src = Color::new(255, 255, 255, 128);
        let out = src.blend_over(dst);
        assert!((out.r as i32 - 128).abs() <= 1);
        assert_eq!(out.a, 255);
    }
}
