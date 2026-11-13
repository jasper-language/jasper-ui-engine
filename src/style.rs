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
// BEGIN VERIFIED COLOR TYPE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
// END VERIFIED COLOR TYPE

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

// BEGIN VERIFIED COLOR CONSTRUCT KERNEL
impl Color {
    /// Exact channels; no premultiplication or clamping.
    /// 通道原样保存；不预乘、不钳制。
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Opaque colour: alpha is 255.
    /// 不透明颜色：alpha 为 255。
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Replaces only alpha; colour channels are kept.
    /// 只替换 alpha，颜色通道保持不变。
    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
}
// END VERIFIED COLOR CONSTRUCT KERNEL

impl Default for Color {
    fn default() -> Self {
        Color::TRANSPARENT
    }
}

/// Font slant requested by Framework; the native path selects a matching injected face.
/// Framework 声明的字体倾斜；原生路径从宿主注入的字面中匹配选择。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
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
    /// CSS numeric weight (400 regular, 700 bold); the native path admits 1..=1000 only.
    /// CSS 数值字重（400 常规、700 粗体）；原生路径只准入 1..=1000。
    pub font_weight: u16,
    pub font_style: FontStyle,
    /// Family name selected by Framework; None keeps the host-injected fallback order.
    /// Framework 选择的字体族名；None 沿用宿主注入的回退顺序。
    pub font_family: Option<String>,
    pub color: Color,
    pub overflow: Overflow,
    /// E2: per-corner radii; paint and the node's own overflow clip follow the rounded box.
    /// E2：四角半径；绘制与节点自身的 overflow 裁剪按圆角盒进行。
    pub corner_radius: CornerRadii,
    /// E3: outer/inner box shadows, painted in order (first on top, as CSS); never laid out.
    /// E3：外/内阴影，按序绘制（第一层在最上，同 CSS）；不参与布局。
    pub box_shadows: Vec<BoxShadow>,
    /// E4: whole-node opacity in 0..=1 (the subtree is composited as one layer).
    /// E4：整节点透明度 0..=1（整个子树作为一层合成）。
    pub opacity: f32,
    /// E9: per-side border overriding `border_width`/`border_color` for that side.
    /// E9：单侧边框，覆盖该侧的 `border_width`/`border_color`。
    pub border_top: Option<BorderSide>,
    pub border_right: Option<BorderSide>,
    pub border_bottom: Option<BorderSide>,
    pub border_left: Option<BorderSide>,
    /// E9: outline outside (or with negative offset inside) the border box; never laid out.
    /// E9：边框盒外（偏移为负时在盒内）的外框；不参与布局。
    pub outline: Option<Outline>,
    /// E9: text decoration lines, drawn in the text colour.
    /// E9：文字装饰线，用文字颜色绘制。
    pub text_decoration: TextDecoration,
    /// E10: pointer hit area extension per side; layout and paint are unchanged.
    /// E10：各边的指针命中区扩展；布局与绘制不变。
    pub hit_slop: Edges,
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
            font_weight: 400,
            font_style: FontStyle::Normal,
            font_family: None,
            color: Color::BLACK,
            overflow: Overflow::Visible,
            corner_radius: CornerRadii::ZERO,
            box_shadows: Vec::new(),
            opacity: 1.0,
            border_top: None,
            border_right: None,
            border_bottom: None,
            border_left: None,
            outline: None,
            text_decoration: TextDecoration::NONE,
            hit_slop: Edges::ZERO,
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

    /// Unclamped: an out-of-range weight is rejected by the native path, not adjusted.
    /// 不钳制：越界字重由原生路径拒绝，而非调整。
    pub fn font(mut self, weight: u16, style: FontStyle) -> Self {
        self.font_weight = weight;
        self.font_style = style;
        self
    }

    /// The native path admits only a family registered by an injected font, matched exactly.
    /// 原生路径只准入注入字体已登记的族名（完全一致）。
    pub fn family(mut self, name: &str) -> Self {
        self.font_family = Some(name.to_owned());
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Same radius on all four corners. Unclamped; invalid values are rejected by the native path.
    /// 四角同一半径。不钳制；非法值由原生路径拒绝。
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = CornerRadii::all(radius);
        self
    }

    pub fn corner_radii(mut self, radii: CornerRadii) -> Self {
        self.corner_radius = radii;
        self
    }

    /// Appends one shadow layer (later layers are painted below earlier ones).
    /// 追加一层阴影（后加的层画在先加的层之下）。
    pub fn shadow(mut self, shadow: BoxShadow) -> Self {
        self.box_shadows.push(shadow);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn border_side(mut self, side: Side, border: Option<BorderSide>) -> Self {
        *match side {
            Side::Top => &mut self.border_top,
            Side::Right => &mut self.border_right,
            Side::Bottom => &mut self.border_bottom,
            Side::Left => &mut self.border_left,
        } = border;
        self
    }

    pub fn outline(mut self, outline: Option<Outline>) -> Self {
        self.outline = outline;
        self
    }

    pub fn text_decoration(mut self, decoration: TextDecoration) -> Self {
        self.text_decoration = decoration;
        self
    }

    pub fn hit_slop(mut self, slop: Edges) -> Self {
        self.hit_slop = slop;
        self
    }

    /// Effective border of one side: the per-side override, else the uniform border.
    /// 某一侧的实际边框：单侧覆盖优先，否则为统一边框。
    pub fn border_of(&self, side: Side) -> BorderSide {
        match side {
            Side::Top => self.border_top,
            Side::Right => self.border_right,
            Side::Bottom => self.border_bottom,
            Side::Left => self.border_left,
        }
        .unwrap_or(BorderSide {
            width: self.border_width,
            color: self.border_color,
        })
    }

    /// Validation of the E2/E3/E4/E9/E10 fields; the native path rejects the whole update
    /// (no clamping) when this fails. Defaults are always valid.
    /// E2/E3/E4/E9/E10 字段的校验；不通过时原生路径拒绝整个更新（不钳制）。默认值总是合法。
    pub fn extended_style_valid(&self) -> bool {
        let length = |v: f32| v.is_finite() && (0.0..=MAX_EFFECT_EXTENT).contains(&v);
        let signed = |v: f32| v.is_finite() && v.abs() <= MAX_EFFECT_EXTENT;
        let r = self.corner_radius;
        [r.top_left, r.top_right, r.bottom_right, r.bottom_left]
            .into_iter()
            .all(length)
            && self.box_shadows.len() <= MAX_BOX_SHADOWS
            && self.box_shadows.iter().all(|s| {
                signed(s.x) && signed(s.y) && length(s.blur) && signed(s.spread)
            })
            && self.opacity.is_finite()
            && (0.0..=1.0).contains(&self.opacity)
            && [
                self.border_top,
                self.border_right,
                self.border_bottom,
                self.border_left,
            ]
            .into_iter()
            .flatten()
            .all(|b| length(b.width))
            && self
                .outline
                .is_none_or(|o| length(o.width) && signed(o.offset))
            && [
                self.hit_slop.top,
                self.hit_slop.right,
                self.hit_slop.bottom,
                self.hit_slop.left,
            ]
            .into_iter()
            .all(length)
    }
}

/// Upper bound (logical px) for every E2/E3/E9/E10 length; bounds blur and ring cost.
/// E2/E3/E9/E10 所有长度的上界（逻辑像素）；限制模糊与外框的绘制代价。
pub const MAX_EFFECT_EXTENT: f32 = 4096.0;
/// Most shadow layers one node may carry.
/// 单个节点最多的阴影层数。
pub const MAX_BOX_SHADOWS: usize = 8;

/// Corner radii in logical px, CSS order: top-left, top-right, bottom-right, bottom-left.
/// Radii larger than the box are scaled down together, as CSS does.
/// 四角半径（逻辑像素），CSS 顺序：左上、右上、右下、左下。超过盒子的半径按 CSS 规则等比缩小。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.top_left == 0.0
            && self.top_right == 0.0
            && self.bottom_right == 0.0
            && self.bottom_left == 0.0
    }
}

/// One shadow layer (CSS `box-shadow` semantics); offsets/blur/spread in logical px.
/// 一层阴影（CSS `box-shadow` 语义）；偏移/模糊/扩展为逻辑像素。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShadow {
    pub x: f32,
    pub y: f32,
    /// Blur radius, >= 0.
    pub blur: f32,
    /// Spread; negative shrinks the shadow.
    pub spread: f32,
    pub color: Color,
    /// Inner shadow (inside the padding box) instead of an outer one.
    pub inset: bool,
}

impl BoxShadow {
    pub const fn new(x: f32, y: f32, blur: f32, spread: f32, color: Color) -> Self {
        Self {
            x,
            y,
            blur,
            spread,
            color,
            inset: false,
        }
    }
}

/// Box side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

/// One solid border side. Border widths take layout space like the uniform border.
/// 一侧实线边框。与统一边框一样，边框宽度占布局空间。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderSide {
    pub width: f32,
    pub color: Color,
}

/// Solid outline ring: `width` thick, starting `offset` px outside the border box (negative:
/// inside). It follows the corner radii and never affects layout or hit testing.
/// 实线外框环：宽 `width`，从边框盒向外 `offset` 像素处开始（负值向内）。随圆角变化，不影响布局与命中。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub width: f32,
    pub offset: f32,
    pub color: Color,
}

/// Text decoration lines.
/// 文字装饰线。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextDecoration {
    pub underline: bool,
    pub line_through: bool,
}

impl TextDecoration {
    pub const NONE: Self = Self {
        underline: false,
        line_through: false,
    };
    pub const UNDERLINE: Self = Self {
        underline: true,
        line_through: false,
    };
    pub const LINE_THROUGH: Self = Self {
        underline: false,
        line_through: true,
    };
}

/// One extended style capability as declared to the runtime (JUI 2 `hello` token) and the
/// `xstyle` keys it covers. Names follow the Jasper theme degradation matrix.
/// 向运行时声明的一项扩展样式能力（JUI 2 `hello` 令牌）及其覆盖的 `xstyle` 键；名称沿用 Jasper 主题降级矩阵。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleCapability {
    pub name: &'static str,
    pub xstyle_keys: &'static [&'static str],
}

/// E2/E3/E4/E9/E10 capabilities implemented by the native engine (`native-blitz-pending`).
/// The legacy prototype engine ignores these fields and declares none of them.
/// 原生引擎（`native-blitz-pending`）实现的 E2/E3/E4/E9/E10 能力；旧原型引擎忽略这些字段，不声明。
pub const EXTENDED_STYLE_CAPABILITIES: &[StyleCapability] = &[
    StyleCapability {
        name: "corner-radius",
        xstyle_keys: &["corner-radius"],
    },
    StyleCapability {
        name: "shadow",
        xstyle_keys: &["shadow"],
    },
    StyleCapability {
        name: "opacity",
        xstyle_keys: &["opacity"],
    },
    StyleCapability {
        name: "border-side",
        xstyle_keys: &["border-top", "border-right", "border-bottom", "border-left"],
    },
    StyleCapability {
        name: "focus-ring",
        xstyle_keys: &["outline"],
    },
    StyleCapability {
        name: "underline",
        xstyle_keys: &["text-decoration"],
    },
    StyleCapability {
        name: "hit-target",
        xstyle_keys: &["hit-slop"],
    },
];

/// `xstyle` key/value rejection.
/// `xstyle` 键值拒绝原因。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XStyleError {
    UnknownKey(String),
    InvalidValue { key: String, value: String },
}

impl std::fmt::Display for XStyleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XStyleError::UnknownKey(key) => write!(f, "unknown xstyle key `{key}`"),
            XStyleError::InvalidValue { key, value } => {
                write!(f, "invalid xstyle value `{key}={value}`")
            }
        }
    }
}

impl std::error::Error for XStyleError {}

fn parse_number(text: &str) -> Option<f32> {
    let text = text.trim();
    // Plain decimal only: no units, no `inf`/`nan` spellings.
    if text.is_empty()
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'))
    {
        return None;
    }
    text.parse::<f32>().ok().filter(|v| v.is_finite())
}

fn parse_color(text: &str) -> Option<Color> {
    let hex = text.trim().strip_prefix('#')?;
    if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Color::new(
        channel(0)?,
        channel(2)?,
        channel(4)?,
        if hex.len() == 8 { channel(6)? } else { 255 },
    ))
}

fn format_color(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

/// One number for all four, or exactly four comma-separated numbers.
fn parse_four(text: &str) -> Option<[f32; 4]> {
    let parts: Vec<&str> = text.split(',').collect();
    match parts.as_slice() {
        [one] => parse_number(one).map(|v| [v; 4]),
        [a, b, c, d] => Some([
            parse_number(a)?,
            parse_number(b)?,
            parse_number(c)?,
            parse_number(d)?,
        ]),
        _ => None,
    }
}

fn parse_shadow(text: &str) -> Option<BoxShadow> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let (fields, inset) = match parts.as_slice() {
        [x, y, blur, spread, color] => ([*x, *y, *blur, *spread, *color], false),
        [x, y, blur, spread, color, "inset"] => ([*x, *y, *blur, *spread, *color], true),
        _ => return None,
    };
    Some(BoxShadow {
        x: parse_number(fields[0])?,
        y: parse_number(fields[1])?,
        blur: parse_number(fields[2])?,
        spread: parse_number(fields[3])?,
        color: parse_color(fields[4])?,
        inset,
    })
}

fn format_number(v: f32) -> String {
    format!("{v}")
}

impl Style {
    /// Applies one JUI 2 `xstyle` pair (see `docs/appfw-style-e2-e10.md` for the grammar).
    /// On error the style is unchanged. Values are validated with `extended_style_valid`.
    /// 应用一条 JUI 2 `xstyle` 键值（语法见 `docs/appfw-style-e2-e10.md`）；出错时样式不变，取值经
    /// `extended_style_valid` 校验。
    pub fn apply_xstyle(&mut self, key: &str, value: &str) -> Result<(), XStyleError> {
        let invalid = || XStyleError::InvalidValue {
            key: key.to_owned(),
            value: value.to_owned(),
        };
        let mut next = self.clone();
        let value_trimmed = value.trim();
        let side = match key {
            "border-top" => Some(Side::Top),
            "border-right" => Some(Side::Right),
            "border-bottom" => Some(Side::Bottom),
            "border-left" => Some(Side::Left),
            _ => None,
        };
        if let Some(side) = side {
            let border = if value_trimmed == "none" {
                None
            } else {
                let (width, color) = value_trimmed.split_once(',').ok_or_else(invalid)?;
                Some(BorderSide {
                    width: parse_number(width).ok_or_else(invalid)?,
                    color: parse_color(color).ok_or_else(invalid)?,
                })
            };
            next = next.border_side(side, border);
        } else {
            match key {
                "corner-radius" => {
                    let [a, b, c, d] = parse_four(value_trimmed).ok_or_else(invalid)?;
                    next.corner_radius = CornerRadii::new(a, b, c, d);
                }
                "shadow" => {
                    next.box_shadows = if value_trimmed == "none" {
                        Vec::new()
                    } else {
                        value_trimmed
                            .split(';')
                            .map(parse_shadow)
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(invalid)?
                    };
                }
                "opacity" => next.opacity = parse_number(value_trimmed).ok_or_else(invalid)?,
                "outline" => {
                    next.outline = if value_trimmed == "none" {
                        None
                    } else {
                        let parts: Vec<&str> = value_trimmed.split(',').collect();
                        let [width, offset, color] = parts.as_slice() else {
                            return Err(invalid());
                        };
                        Some(Outline {
                            width: parse_number(width).ok_or_else(invalid)?,
                            offset: parse_number(offset).ok_or_else(invalid)?,
                            color: parse_color(color).ok_or_else(invalid)?,
                        })
                    };
                }
                "text-decoration" => {
                    let mut decoration = TextDecoration::NONE;
                    if value_trimmed != "none" {
                        for line in value_trimmed.split(',').map(str::trim) {
                            match line {
                                "underline" if !decoration.underline => {
                                    decoration.underline = true
                                }
                                "line-through" if !decoration.line_through => {
                                    decoration.line_through = true
                                }
                                _ => return Err(invalid()),
                            }
                        }
                    }
                    next.text_decoration = decoration;
                }
                "hit-slop" => {
                    let [t, r, b, l] = parse_four(value_trimmed).ok_or_else(invalid)?;
                    next.hit_slop = Edges::new(t, r, b, l);
                }
                _ => return Err(XStyleError::UnknownKey(key.to_owned())),
            }
        }
        if !next.extended_style_valid() {
            return Err(invalid());
        }
        *self = next;
        Ok(())
    }

    /// The `xstyle` pairs of every extended field that differs from its default, in a fixed
    /// key order; `apply_xstyle` on a default style reproduces them.
    /// 与默认值不同的每个扩展字段对应的 `xstyle` 键值（固定键序）；在默认样式上逐条 `apply_xstyle` 可还原。
    pub fn xstyle_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        let r = self.corner_radius;
        if !r.is_zero() {
            pairs.push((
                "corner-radius",
                if r.top_left == r.top_right
                    && r.top_left == r.bottom_right
                    && r.top_left == r.bottom_left
                {
                    format_number(r.top_left)
                } else {
                    [r.top_left, r.top_right, r.bottom_right, r.bottom_left]
                        .map(format_number)
                        .join(",")
                },
            ));
        }
        if !self.box_shadows.is_empty() {
            pairs.push((
                "shadow",
                self.box_shadows
                    .iter()
                    .map(|s| {
                        let mut text = format!(
                            "{},{},{},{},{}",
                            format_number(s.x),
                            format_number(s.y),
                            format_number(s.blur),
                            format_number(s.spread),
                            format_color(s.color)
                        );
                        if s.inset {
                            text.push_str(",inset");
                        }
                        text
                    })
                    .collect::<Vec<_>>()
                    .join(";"),
            ));
        }
        if self.opacity != 1.0 {
            pairs.push(("opacity", format_number(self.opacity)));
        }
        for (key, side) in [
            ("border-top", self.border_top),
            ("border-right", self.border_right),
            ("border-bottom", self.border_bottom),
            ("border-left", self.border_left),
        ] {
            if let Some(b) = side {
                pairs.push((key, format!("{},{}", format_number(b.width), format_color(b.color))));
            }
        }
        if let Some(o) = self.outline {
            pairs.push((
                "outline",
                format!(
                    "{},{},{}",
                    format_number(o.width),
                    format_number(o.offset),
                    format_color(o.color)
                ),
            ));
        }
        let d = self.text_decoration;
        if d != TextDecoration::NONE {
            let mut lines = Vec::new();
            if d.underline {
                lines.push("underline");
            }
            if d.line_through {
                lines.push("line-through");
            }
            pairs.push(("text-decoration", lines.join(",")));
        }
        let h = self.hit_slop;
        if h != Edges::ZERO {
            pairs.push((
                "hit-slop",
                if h.top == h.right && h.top == h.bottom && h.top == h.left {
                    format_number(h.top)
                } else {
                    [h.top, h.right, h.bottom, h.left].map(format_number).join(",")
                },
            ));
        }
        pairs
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
    fn extended_defaults_are_valid_and_neutral() {
        let style = Style::default();
        assert!(style.extended_style_valid());
        assert!(style.xstyle_pairs().is_empty());
        assert_eq!(style.opacity, 1.0);
        assert!(style.corner_radius.is_zero());
        assert_eq!(
            style.border_of(Side::Top),
            BorderSide {
                width: 0.0,
                color: Color::TRANSPARENT
            }
        );
    }

    #[test]
    fn extended_validation_rejects_out_of_range() {
        let ok = Style::default();
        for bad in [
            ok.clone().corner_radius(-1.0),
            ok.clone().corner_radius(f32::NAN),
            ok.clone().corner_radius(MAX_EFFECT_EXTENT + 1.0),
            ok.clone().opacity(1.01),
            ok.clone().opacity(-0.1),
            ok.clone().opacity(f32::INFINITY),
            ok.clone().shadow(BoxShadow::new(0.0, 0.0, -1.0, 0.0, Color::BLACK)),
            ok.clone().shadow(BoxShadow::new(f32::NAN, 0.0, 1.0, 0.0, Color::BLACK)),
            ok.clone().border_side(
                Side::Left,
                Some(BorderSide {
                    width: -1.0,
                    color: Color::BLACK,
                }),
            ),
            ok.clone().outline(Some(Outline {
                width: -2.0,
                offset: 0.0,
                color: Color::BLACK,
            })),
            ok.clone().outline(Some(Outline {
                width: 2.0,
                offset: f32::NAN,
                color: Color::BLACK,
            })),
            ok.clone().hit_slop(Edges::new(0.0, -1.0, 0.0, 0.0)),
        ] {
            assert!(!bad.extended_style_valid(), "{bad:?}");
        }
        let mut many = ok.clone();
        for _ in 0..=MAX_BOX_SHADOWS {
            many = many.shadow(BoxShadow::new(0.0, 1.0, 2.0, 0.0, Color::BLACK));
        }
        assert!(!many.extended_style_valid());
        // Negative spread and negative outline offset are meaningful, not errors.
        assert!(ok
            .clone()
            .shadow(BoxShadow::new(-2.0, 3.0, 4.0, -1.0, Color::BLACK))
            .outline(Some(Outline {
                width: 2.0,
                offset: -3.0,
                color: Color::BLACK
            }))
            .extended_style_valid());
    }

    #[test]
    fn border_side_overrides_only_its_side() {
        let style = Style::default().border(1.0, Color::RED).border_side(
            Side::Bottom,
            Some(BorderSide {
                width: 3.0,
                color: Color::BLUE,
            }),
        );
        assert_eq!(style.border_of(Side::Bottom).width, 3.0);
        assert_eq!(style.border_of(Side::Bottom).color, Color::BLUE);
        assert_eq!(style.border_of(Side::Top).width, 1.0);
        assert_eq!(style.border_of(Side::Left).color, Color::RED);
    }

    #[test]
    fn xstyle_parses_every_key() {
        let mut s = Style::default();
        s.apply_xstyle("corner-radius", "8").unwrap();
        assert_eq!(s.corner_radius, CornerRadii::all(8.0));
        s.apply_xstyle("corner-radius", "8,8,4,4").unwrap();
        assert_eq!(s.corner_radius, CornerRadii::new(8.0, 8.0, 4.0, 4.0));
        s.apply_xstyle("shadow", "0,1,2,0,#00000040;0,4,8,-2,#0000001a,inset")
            .unwrap();
        assert_eq!(
            s.box_shadows,
            vec![
                BoxShadow::new(0.0, 1.0, 2.0, 0.0, Color::new(0, 0, 0, 0x40)),
                BoxShadow {
                    inset: true,
                    ..BoxShadow::new(0.0, 4.0, 8.0, -2.0, Color::new(0, 0, 0, 0x1a))
                },
            ]
        );
        s.apply_xstyle("opacity", "0.5").unwrap();
        assert_eq!(s.opacity, 0.5);
        s.apply_xstyle("border-top", "1,#11223344").unwrap();
        assert_eq!(
            s.border_top,
            Some(BorderSide {
                width: 1.0,
                color: Color::new(0x11, 0x22, 0x33, 0x44)
            })
        );
        s.apply_xstyle("border-left", "2,#aabbcc").unwrap();
        assert_eq!(s.border_left.unwrap().color, Color::rgb(0xaa, 0xbb, 0xcc));
        s.apply_xstyle("outline", "2,2,#0a84ff").unwrap();
        assert_eq!(
            s.outline,
            Some(Outline {
                width: 2.0,
                offset: 2.0,
                color: Color::rgb(0x0a, 0x84, 0xff)
            })
        );
        s.apply_xstyle("text-decoration", "underline").unwrap();
        assert_eq!(s.text_decoration, TextDecoration::UNDERLINE);
        s.apply_xstyle("text-decoration", "underline,line-through")
            .unwrap();
        assert!(s.text_decoration.underline && s.text_decoration.line_through);
        s.apply_xstyle("hit-slop", "8").unwrap();
        assert_eq!(s.hit_slop, Edges::all(8.0));
        s.apply_xstyle("hit-slop", "1,2,3,4").unwrap();
        assert_eq!(s.hit_slop, Edges::new(1.0, 2.0, 3.0, 4.0));

        // Round trip through the canonical pairs.
        let mut copy = Style::default();
        for (key, value) in s.xstyle_pairs() {
            copy.apply_xstyle(key, &value).unwrap();
        }
        assert_eq!(copy, s);

        // `none` clears.
        for key in ["shadow", "border-top", "border-left", "outline", "text-decoration"] {
            s.apply_xstyle(key, "none").unwrap();
        }
        assert!(s.box_shadows.is_empty() && s.border_top.is_none() && s.outline.is_none());
        assert_eq!(s.text_decoration, TextDecoration::NONE);
    }

    #[test]
    fn xstyle_rejects_without_change() {
        let mut s = Style::default().corner_radius(3.0);
        let before = s.clone();
        for (key, value) in [
            ("corner-radius", "8,8"),
            ("corner-radius", "-1"),
            ("corner-radius", "8px"),
            ("corner-radius", "inf"),
            ("shadow", "0,1,2,#000"),
            ("shadow", "0,1,-2,0,#000000"),
            ("shadow", "0,1,2,0,#000000,outset"),
            ("opacity", "2"),
            ("opacity", "nan"),
            ("border-top", "1"),
            ("border-top", "1,red"),
            ("outline", "2,#000000"),
            ("text-decoration", "overline"),
            ("text-decoration", "underline,underline"),
            ("hit-slop", "-4"),
        ] {
            assert!(
                matches!(s.apply_xstyle(key, value), Err(XStyleError::InvalidValue { .. })),
                "{key}={value}"
            );
            assert_eq!(s, before);
        }
        assert_eq!(
            s.apply_xstyle("blur", "4"),
            Err(XStyleError::UnknownKey("blur".into()))
        );
        let mut nine = String::new();
        for i in 0..=MAX_BOX_SHADOWS {
            if i > 0 {
                nine.push(';');
            }
            nine.push_str("0,1,2,0,#000000");
        }
        assert!(s.apply_xstyle("shadow", &nine).is_err());
    }

    #[test]
    fn capability_table_covers_every_xstyle_key_once() {
        let mut keys: Vec<&str> = EXTENDED_STYLE_CAPABILITIES
            .iter()
            .flat_map(|c| c.xstyle_keys.iter().copied())
            .collect();
        let count = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), count);
        let mut s = Style::default();
        for key in keys {
            // Every declared key is understood by the parser (here: cleared/neutral values).
            let value = match key {
                "corner-radius" | "hit-slop" => "0",
                "opacity" => "1",
                _ => "none",
            };
            s.apply_xstyle(key, value).unwrap();
        }
        assert_eq!(s, Style::default());
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
