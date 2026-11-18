//! Portable style subset (PSS) used by the engine.
//! 可移植样式子集；不是完整 CSS，公共语义由 Jasper UI 定义，这里只承载引擎需要的部分。

use crate::nostd_prelude::*;
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
    Grid,
    Table,
    TableRow,
    TableCell,
    None,
}

/// CSS-like positioning mode. Static preserves the historical flow-only behavior.
/// 类 CSS 定位模式；Static 保持原有纯流式行为的默认语义。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// One track in an explicit grid template.
/// 显式 Grid 模板中的一条轨道。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridTrack {
    Auto,
    Px(f32),
    Percent(f32),
    Fr(f32),
}

/// A grid line: automatic placement, an explicit integer line, or a span.
/// Grid 线：自动放置、显式整数线或跨若干轨道。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridLine {
    #[default]
    Auto,
    Line(i32),
    Span(u32),
}

/// Four logical-pixel lengths used for positioning insets.
/// 定位内边距使用的四个逻辑像素长度。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inset {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

impl Inset {
    pub const ZERO: Self = Self {
        top: Length::Auto,
        right: Length::Auto,
        bottom: Length::Auto,
        left: Length::Auto,
    };

    pub const fn all(value: Length) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub const fn new(top: Length, right: Length, bottom: Length, left: Length) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

/// Main-axis direction for flex layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    #[default]
    Column,
}

/// Cross-line wrapping for flex layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Cross-axis alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Baseline,
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
    SpaceAround,
    SpaceEvenly,
}

/// Horizontal text alignment inside a text line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
    Justify,
}

/// Explicit bidirectional text direction. `Auto` keeps the Blitz default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
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
    pub flex_wrap: FlexWrap,
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
    pub flex_shrink: f32,
    /// Positioning and stacking. Defaults are neutral and preserve historical flow order.
    /// 定位与层级；默认值中立，保持历史流式顺序。
    pub position: Position,
    pub inset: Inset,
    pub z_index: Option<i32>,
    pub order: i32,
    /// Explicit grid tracks. None keeps auto placement and the browser-compatible implicit grid.
    /// 显式 Grid 轨道；None 保留自动放置与浏览器兼容的隐式轨道。
    pub grid_columns: Option<Vec<GridTrack>>,
    pub grid_rows: Option<Vec<GridTrack>>,
    pub grid_column: GridLine,
    pub grid_row: GridLine,
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
    /// E8: 2D transform (translate, rotate, scale about an origin) applied to paint and hit
    /// testing of the node and its subtree; layout and `node_bounds` are unchanged.
    /// E8：二维变换（绕原点的平移、旋转、缩放），作用于节点及其子树的绘制与命中；不影响布局与 `node_bounds`。
    pub transform: Transform2D,
    /// E7: transitions the engine runs when a later update changes a covered property. Empty:
    /// every change is immediate (the default, with no per-frame work).
    /// E7：之后的更新改变所覆盖属性时由引擎执行的过渡。为空时所有变化立即生效（默认，无逐帧开销）。
    pub transitions: Vec<Transition>,
    /// E6: gradient painted above `background` and below borders, clipped like the background
    /// (rounded border box). None: no gradient (the default).
    /// E6：绘制在 `background` 之上、边框之下的渐变，与背景同样裁剪（圆角边框盒）。None 为无渐变（默认）。
    pub background_gradient: Option<Gradient>,
    /// E5: material (frosted glass) under the node's own background: what is painted behind the
    /// node, blurred, saturated and tinted, clipped to the rounded border box. None: no material.
    /// E5：节点自身背景之下的材质（毛玻璃）：节点背后已绘内容经模糊、饱和度与着色，按圆角边框盒裁剪。None 为无材质。
    pub backdrop: Option<Backdrop>,
    /// E15: line height of the node's own text (CSS `line-height`). None: the font's normal
    /// line height (the default, unchanged layout). Applies to Text nodes only; never
    /// transitioned (a change is immediate).
    /// E15：节点自身文字的行高（CSS `line-height`）。None 为字体的正常行高（默认，布局不变）。只作用于 Text 节点；
    /// 不参与过渡（变化立即生效）。
    pub line_height: Option<LineHeight>,
    pub text_align: TextAlign,
    pub text_direction: TextDirection,
    /// E15: vertical placement of a Text node's text inside its content box when the box is
    /// taller than the text (fixed height). None: the node's own flex alignment decides (top by
    /// default, unchanged pixels). Never changes the node's layout size, hit area or
    /// `node_bounds`; applies to Text nodes only; never transitioned.
    /// E15：Text 节点内容盒高于文字（固定高度）时文字的垂直位置。None 时由节点自身的 flex 对齐决定（默认靠上，像素不变）。
    /// 从不改变节点布局尺寸、命中区与 `node_bounds`；只作用于 Text 节点；不参与过渡。
    pub text_align_vertical: Option<TextAlignVertical>,
    /// P0-A image content: a host-injected resource (bitmap or SVG) drawn in the content box,
    /// under the node's children and text, clipped to the (rounded) content box. The image never
    /// changes layout: the node is sized by its own style exactly like a Rect. None: no image.
    /// P0-A 图片内容：宿主注入的资源（位图或 SVG）画在内容盒内，位于子节点与文字之下，按（圆角）内容盒裁剪。
    /// 图片从不改变布局：节点尺寸只由自身样式决定，与 Rect 相同。None 为无图片（默认）。
    pub image: Option<ImageStyle>,
    /// Overlay layer (issue #1 P0-A): lifts the node and its subtree out of the parent's clip
    /// and paint order, anchors and places it, and gives it modal / dismiss / focus behaviour
    /// (see `crate::overlay`). None: an ordinary node (the default). Never transitioned.
    /// 浮层层（issue #1 P0-A）：使节点及其子树脱离父节点裁剪与绘制顺序，锚定并定位，带模态/关闭请求/焦点行为
    /// （见 `crate::overlay`）。None 为普通节点（默认）。不参与过渡。
    pub overlay: Option<crate::overlay::OverlaySpec>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            direction: FlexDirection::Column,
            flex_wrap: FlexWrap::NoWrap,
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
            flex_shrink: 1.0,
            position: Position::Static,
            inset: Inset::ZERO,
            z_index: None,
            order: 0,
            grid_columns: None,
            grid_rows: None,
            grid_column: GridLine::Auto,
            grid_row: GridLine::Auto,
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
            transform: Transform2D::IDENTITY,
            transitions: Vec::new(),
            background_gradient: None,
            backdrop: None,
            line_height: None,
            text_align: TextAlign::Start,
            text_direction: TextDirection::Auto,
            text_align_vertical: None,
            image: None,
            overlay: None,
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

    pub fn flex_wrap(mut self, wrap: FlexWrap) -> Self {
        self.flex_wrap = wrap;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink.max(0.0);
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

    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.text_align = align;
        self
    }

    pub fn text_direction(mut self, direction: TextDirection) -> Self {
        self.text_direction = direction;
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow.max(0.0);
        self
    }

    pub fn position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    pub fn inset(mut self, inset: Inset) -> Self {
        self.inset = inset;
        self
    }

    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = Some(z_index);
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn grid_columns(mut self, tracks: Vec<GridTrack>) -> Self {
        self.grid_columns = Some(tracks);
        self
    }

    pub fn grid_rows(mut self, tracks: Vec<GridTrack>) -> Self {
        self.grid_rows = Some(tracks);
        self
    }

    pub fn grid_column(mut self, line: GridLine) -> Self {
        self.grid_column = line;
        self
    }

    pub fn grid_row(mut self, line: GridLine) -> Self {
        self.grid_row = line;
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

    pub fn transform(mut self, transform: Transform2D) -> Self {
        self.transform = transform;
        self
    }

    /// Appends one transition rule (a later rule wins for a property both cover, as in CSS).
    /// 追加一条过渡规则（两条规则覆盖同一属性时以后者为准，同 CSS）。
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transitions.push(transition);
        self
    }

    pub fn background_gradient(mut self, gradient: Option<Gradient>) -> Self {
        self.background_gradient = gradient;
        self
    }

    pub fn backdrop(mut self, backdrop: Option<Backdrop>) -> Self {
        self.backdrop = backdrop;
        self
    }

    pub fn line_height(mut self, line_height: Option<LineHeight>) -> Self {
        self.line_height = line_height;
        self
    }

    /// P0-A image content (see `ImageStyle`).
    /// P0-A 图片内容（见 `ImageStyle`）。
    pub fn image(mut self, image: Option<ImageStyle>) -> Self {
        self.image = image;
        self
    }

    pub fn text_align_vertical(mut self, align: Option<TextAlignVertical>) -> Self {
        self.text_align_vertical = align;
        self
    }

    /// The rule that governs `property`: the last one covering it.
    /// 决定 `property` 的规则：覆盖它的最后一条。
    pub fn transition_for(&self, property: AnimatedProperties) -> Option<&Transition> {
        self.transitions
            .iter()
            .rev()
            .find(|t| t.properties.contains(property))
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
            && self.transform.valid()
            && self.transitions.len() <= MAX_TRANSITIONS
            && self.transitions.iter().all(Transition::valid)
            && self.background_gradient.as_ref().is_none_or(Gradient::valid)
            && self.backdrop.as_ref().is_none_or(Backdrop::valid)
            && self.line_height.as_ref().is_none_or(LineHeight::valid)
            && self.flex_shrink.is_finite()
            && self.flex_shrink >= 0.0
            && self.image.as_ref().is_none_or(ImageStyle::valid)
            && self.overlay.as_ref().is_none_or(crate::overlay::OverlaySpec::valid)
            && [
                self.inset.top,
                self.inset.right,
                self.inset.bottom,
                self.inset.left,
            ]
            .into_iter()
            .all(|v| match v {
                Length::Auto => true,
                Length::Px(v) | Length::Percent(v) => v.is_finite() && v >= 0.0,
            })
            && self.grid_columns.as_deref().is_none_or(valid_grid_tracks)
            && self.grid_rows.as_deref().is_none_or(valid_grid_tracks)
            && valid_grid_line(self.grid_column)
            && valid_grid_line(self.grid_row)
    }
}

pub(crate) fn valid_grid_tracks(tracks: &[GridTrack]) -> bool {
    tracks.len() <= 256
        && tracks.iter().all(|track| match track {
            GridTrack::Auto => true,
            GridTrack::Px(v) | GridTrack::Percent(v) => v.is_finite() && *v >= 0.0,
            GridTrack::Fr(v) => v.is_finite() && *v > 0.0,
        })
}

fn valid_grid_line(line: GridLine) -> bool {
    match line {
        GridLine::Auto => true,
        GridLine::Line(v) => v != 0,
        GridLine::Span(v) => v > 0,
    }
}

/// Upper bound (logical px) for every E2/E3/E9/E10 length; bounds blur and ring cost.
/// E2/E3/E9/E10 所有长度的上界（逻辑像素）；限制模糊与外框的绘制代价。
pub const MAX_EFFECT_EXTENT: f32 = 4096.0;
/// Most shadow layers one node may carry.
/// 单个节点最多的阴影层数。
pub const MAX_BOX_SHADOWS: usize = 8;

/// Bound on |translate| (logical px) of an E8 transform.
/// E8 变换平移量绝对值的上界（逻辑像素）。
pub const MAX_TRANSFORM_TRANSLATE: f32 = 16384.0;
/// Bound on |scale| of an E8 transform.
/// E8 变换缩放绝对值的上界。
pub const MAX_TRANSFORM_SCALE: f32 = 64.0;
/// Bound on |rotate| (degrees) of an E8 transform (ten full turns).
/// E8 变换旋转角绝对值的上界（度，十整圈）。
pub const MAX_TRANSFORM_ROTATE: f32 = 3600.0;
/// Bound on |origin| (fraction of the border box) of an E8 transform.
/// E8 变换原点绝对值的上界（边框盒的比例）。
pub const MAX_TRANSFORM_ORIGIN: f32 = 4.0;

/// E8 2D transform in CSS order: `translate(tx, ty) rotate(r) scale(sx, sy)` about the origin
/// (`transform-origin`, as a fraction of the node's border box; 0.5/0.5 is the centre). Positive
/// rotation is clockwise on screen (y down), in degrees. It never affects layout.
/// E8 二维变换，CSS 顺序：绕原点（`transform-origin`，边框盒的比例；0.5/0.5 为中心）做
/// `translate(tx, ty) rotate(r) scale(sx, sy)`。正角度为屏幕上的顺时针（y 向下），单位为度。不影响布局。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    pub translate_x: f32,
    pub translate_y: f32,
    pub rotate: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform2D {
    pub const IDENTITY: Self = Self {
        translate_x: 0.0,
        translate_y: 0.0,
        rotate: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        origin_x: 0.5,
        origin_y: 0.5,
    };

    pub const fn translate(x: f32, y: f32) -> Self {
        Self {
            translate_x: x,
            translate_y: y,
            ..Self::IDENTITY
        }
    }

    pub const fn scale(s: f32) -> Self {
        Self {
            scale_x: s,
            scale_y: s,
            ..Self::IDENTITY
        }
    }

    pub const fn rotate(degrees: f32) -> Self {
        Self {
            rotate: degrees,
            ..Self::IDENTITY
        }
    }

    pub const fn scale_xy(mut self, x: f32, y: f32) -> Self {
        self.scale_x = x;
        self.scale_y = y;
        self
    }

    pub const fn with_origin(mut self, x: f32, y: f32) -> Self {
        self.origin_x = x;
        self.origin_y = y;
        self
    }

    /// No visible effect (the origin is irrelevant then).
    /// 无可见效果（此时原点无关）。
    pub fn is_identity(&self) -> bool {
        self.translate_x == 0.0
            && self.translate_y == 0.0
            && self.rotate == 0.0
            && self.scale_x == 1.0
            && self.scale_y == 1.0
    }

    pub fn valid(&self) -> bool {
        let within = |v: f32, bound: f32| v.is_finite() && v.abs() <= bound;
        within(self.translate_x, MAX_TRANSFORM_TRANSLATE)
            && within(self.translate_y, MAX_TRANSFORM_TRANSLATE)
            && within(self.rotate, MAX_TRANSFORM_ROTATE)
            && within(self.scale_x, MAX_TRANSFORM_SCALE)
            && within(self.scale_y, MAX_TRANSFORM_SCALE)
            && within(self.origin_x, MAX_TRANSFORM_ORIGIN)
            && within(self.origin_y, MAX_TRANSFORM_ORIGIN)
    }

    /// Affine `[a, b, c, d, e, f]` (x' = a·x + c·y + e, y' = b·x + d·y + f) of a box whose
    /// border box starts at (`left`, `top`) and is `width`×`height`, in the same coordinates.
    /// 左上角在 (`left`, `top`)、尺寸 `width`×`height` 的边框盒在同一坐标系中的仿射矩阵
    /// `[a, b, c, d, e, f]`（x' = a·x + c·y + e，y' = b·x + d·y + f）。
    pub fn matrix(&self, left: f64, top: f64, width: f64, height: f64) -> [f64; 6] {
        let ox = left + f64::from(self.origin_x) * width;
        let oy = top + f64::from(self.origin_y) * height;
        let (sin, cos) = f64::from(self.rotate).to_radians().sin_cos();
        let (sx, sy) = (f64::from(self.scale_x), f64::from(self.scale_y));
        // R·S
        let (a, b, c, d) = (cos * sx, sin * sx, -sin * sy, cos * sy);
        // Tr(o)·Tr(t)·R·S·Tr(-o)
        let e = ox + f64::from(self.translate_x) - (a * ox + c * oy);
        let f = oy + f64::from(self.translate_y) - (b * ox + d * oy);
        [a, b, c, d, e, f]
    }
}

/// Most transition rules one node may carry.
/// 单个节点最多的过渡规则数。
pub const MAX_TRANSITIONS: usize = 16;
/// Bound on a transition duration or delay, in milliseconds.
/// 过渡时长与延迟的上界（毫秒）。
pub const MAX_TRANSITION_MS: f32 = 60_000.0;
/// Bound on |y1|, |y2| of a cubic Bézier easing.
/// 三次贝塞尔缓动 |y1|、|y2| 的上界。
pub const MAX_BEZIER_Y: f32 = 8.0;
/// Bounds on spring mass, stiffness and damping (each must also be > 0).
/// 弹簧质量、刚度、阻尼的上界（且都须 > 0）。
pub const MAX_SPRING_MASS: f32 = 1000.0;
pub const MAX_SPRING_STIFFNESS: f32 = 100_000.0;
pub const MAX_SPRING_DAMPING: f32 = 100_000.0;

/// Set of animatable properties a transition rule covers.
/// 一条过渡规则覆盖的可动画属性集合。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnimatedProperties(pub u16);

impl AnimatedProperties {
    pub const NONE: Self = Self(0);
    /// `background`
    pub const BACKGROUND: Self = Self(1 << 0);
    /// Text `color`.
    pub const COLOR: Self = Self(1 << 1);
    /// `border_color` and the colour of every per-side border.
    pub const BORDER_COLOR: Self = Self(1 << 2);
    pub const OPACITY: Self = Self(1 << 3);
    pub const CORNER_RADIUS: Self = Self(1 << 4);
    /// Every shadow layer (offsets, blur, spread, colour).
    pub const SHADOW: Self = Self(1 << 5);
    /// Outline width, offset and colour.
    pub const OUTLINE: Self = Self(1 << 6);
    /// Laid-out width (only between two `Px` or two `Percent` values).
    pub const WIDTH: Self = Self(1 << 7);
    pub const HEIGHT: Self = Self(1 << 8);
    /// E8 transform (translate, rotate, scale, origin).
    pub const TRANSFORM: Self = Self(1 << 9);
    /// E5 material: blur, saturation, tint and stroke (only while both ends have a backdrop).
    /// E5 材质：模糊、饱和度、着色与描边（仅当两端都有材质时）。
    pub const BACKDROP: Self = Self(1 << 10);
    /// E6 gradient: geometry and colour stops (only between gradients of the same kind and stop
    /// count).
    /// E6 渐变：几何与色标（仅在同类、同色标数的渐变之间）。
    pub const GRADIENT: Self = Self(1 << 11);
    pub const ALL: Self = Self((1 << 12) - 1);
    /// Properties that move or resize content; reduce-motion always makes these immediate.
    /// 移动或改变内容尺寸的属性；减少动态下这些属性总是立即生效。
    pub const MOVEMENT: Self = Self(Self::WIDTH.0 | Self::HEIGHT.0 | Self::TRANSFORM.0);

    /// Each single property with its `xstyle` name, in canonical order.
    /// 每个单项属性及其 `xstyle` 名称，按规范顺序。
    pub const NAMED: [(&'static str, Self); 12] = [
        ("background", Self::BACKGROUND),
        ("color", Self::COLOR),
        ("border-color", Self::BORDER_COLOR),
        ("opacity", Self::OPACITY),
        ("corner-radius", Self::CORNER_RADIUS),
        ("shadow", Self::SHADOW),
        ("outline", Self::OUTLINE),
        ("width", Self::WIDTH),
        ("height", Self::HEIGHT),
        ("transform", Self::TRANSFORM),
        ("backdrop", Self::BACKDROP),
        ("gradient", Self::GRADIENT),
    ];

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        other.0 != 0 && self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Timing of a transition.
/// 过渡的时间曲线。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    /// CSS `cubic-bezier(x1, y1, x2, y2)` over the rule's duration; x1, x2 in 0..=1.
    /// CSS `cubic-bezier(x1, y1, x2, y2)`，持续规则时长；x1、x2 ∈ 0..=1。
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
    /// Damped spring (mass, stiffness, damping, all > 0) solved analytically per component;
    /// the rule's duration is ignored: it ends when its decay envelope is within tolerance.
    /// 阻尼弹簧（质量、刚度、阻尼，均 > 0），逐分量解析求解；忽略规则时长：衰减包络进入容差即结束。
    Spring {
        mass: f32,
        stiffness: f32,
        damping: f32,
    },
}

impl Easing {
    pub const LINEAR: Self = Self::CubicBezier {
        x1: 0.0,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
    };
    pub const EASE: Self = Self::CubicBezier {
        x1: 0.25,
        y1: 0.1,
        x2: 0.25,
        y2: 1.0,
    };
    pub const EASE_IN: Self = Self::CubicBezier {
        x1: 0.42,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
    };
    pub const EASE_OUT: Self = Self::CubicBezier {
        x1: 0.0,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    };
    pub const EASE_IN_OUT: Self = Self::CubicBezier {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    };

    pub const fn cubic(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self::CubicBezier { x1, y1, x2, y2 }
    }

    pub const fn spring(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self::Spring {
            mass,
            stiffness,
            damping,
        }
    }

    pub fn valid(&self) -> bool {
        match *self {
            Easing::CubicBezier { x1, y1, x2, y2 } => {
                [x1, y1, x2, y2].iter().all(|v| v.is_finite())
                    && (0.0..=1.0).contains(&x1)
                    && (0.0..=1.0).contains(&x2)
                    && y1.abs() <= MAX_BEZIER_Y
                    && y2.abs() <= MAX_BEZIER_Y
            }
            Easing::Spring {
                mass,
                stiffness,
                damping,
            } => {
                let positive = |v: f32, max: f32| v.is_finite() && v > 0.0 && v <= max;
                positive(mass, MAX_SPRING_MASS)
                    && positive(stiffness, MAX_SPRING_STIFFNESS)
                    && positive(damping, MAX_SPRING_DAMPING)
            }
        }
    }
}

/// Reduce-motion replacement of a rule: a cross-fade timing for its non-movement properties.
/// 规则的减少动态替代：其非移动属性所用的淡入淡出时间曲线。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReducedTransition {
    pub duration_ms: f32,
    /// Cubic Bézier only; a spring is not a reduced motion.
    /// 只能是三次贝塞尔；弹簧不算减少动态。
    pub easing: Easing,
}

/// One transition rule: which properties, how long, which curve, after which delay.
/// 一条过渡规则：哪些属性、多长、什么曲线、延迟多久。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub properties: AnimatedProperties,
    /// Milliseconds (ignored by a spring).
    pub duration_ms: f32,
    pub delay_ms: f32,
    pub easing: Easing,
    /// Used under reduce-motion for the non-movement properties; None: immediate.
    /// 减少动态下用于非移动属性；None 表示立即生效。
    pub reduced: Option<ReducedTransition>,
}

impl Transition {
    pub const fn new(properties: AnimatedProperties, duration_ms: f32, easing: Easing) -> Self {
        Self {
            properties,
            duration_ms,
            delay_ms: 0.0,
            easing,
            reduced: None,
        }
    }

    pub const fn spring(
        properties: AnimatedProperties,
        mass: f32,
        stiffness: f32,
        damping: f32,
    ) -> Self {
        Self::new(properties, 0.0, Easing::spring(mass, stiffness, damping))
    }

    pub const fn delay(mut self, delay_ms: f32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub const fn reduced(mut self, duration_ms: f32, easing: Easing) -> Self {
        self.reduced = Some(ReducedTransition {
            duration_ms,
            easing,
        });
        self
    }

    pub fn valid(&self) -> bool {
        let ms = |v: f32| v.is_finite() && (0.0..=MAX_TRANSITION_MS).contains(&v);
        !self.properties.is_empty()
            && AnimatedProperties::ALL.contains(self.properties)
            && ms(self.duration_ms)
            && ms(self.delay_ms)
            && self.easing.valid()
            && self.reduced.is_none_or(|r| {
                ms(r.duration_ms)
                    && matches!(r.easing, Easing::CubicBezier { .. })
                    && r.easing.valid()
            })
    }
}

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

/// E15 line height: absolute logical px, or a multiple of the font size.
/// E15 行高：逻辑像素绝对值，或字号的倍数。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    /// `0 < px <= MAX_EFFECT_EXTENT`.
    Px(f32),
    /// `0 < multiple <= MAX_LINE_HEIGHT_MULTIPLE`.
    Multiple(f32),
}

/// Upper bound of an E15 line-height multiple.
/// E15 行高倍数的上界。
pub const MAX_LINE_HEIGHT_MULTIPLE: f32 = 16.0;

impl LineHeight {
    pub fn valid(&self) -> bool {
        match *self {
            LineHeight::Px(v) => v.is_finite() && v > 0.0 && v <= MAX_EFFECT_EXTENT,
            LineHeight::Multiple(v) => v.is_finite() && v > 0.0 && v <= MAX_LINE_HEIGHT_MULTIPLE,
        }
    }
}

/// E15 vertical alignment of a Text node's text in its content box.
/// E15 Text 节点文字在内容盒中的垂直对齐。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlignVertical {
    Top,
    Center,
    Bottom,
}

/// Most colour stops of one E6 gradient.
/// E6 渐变的色标数上限。
pub const MAX_GRADIENT_STOPS: usize = 16;

/// Geometry of an E6 gradient.
/// E6 渐变的几何。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientShape {
    /// CSS `linear-gradient(<angle>deg, ...)`: 0 points to the top, 90 to the right (clockwise);
    /// the gradient line spans the border box corner to corner along that direction (CSS).
    /// CSS `linear-gradient(<angle>deg, ...)`：0 指向上方，90 指向右方（顺时针）；渐变线按 CSS 跨越边框盒。
    Linear { angle: f32 },
    /// CSS `radial-gradient`: centre as fractions of the border box (0.5/0.5 is the middle);
    /// `radius` None is the CSS default ellipse reaching the farthest corner, Some(r) a circle of
    /// radius r logical px.
    /// CSS `radial-gradient`：中心为边框盒比例（0.5/0.5 为正中）；`radius` 为 None 时是 CSS 默认的
    /// 到最远角的椭圆，Some(r) 为半径 r 逻辑像素的圆。
    Radial {
        center_x: f32,
        center_y: f32,
        radius: Option<f32>,
    },
}

/// One colour stop; `position` is a fraction 0..=1 of the gradient line (or ray).
/// 一个色标；`position` 为渐变线（或半径）上的比例 0..=1。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    pub position: f32,
    pub color: Color,
}

impl GradientStop {
    pub const fn new(position: f32, color: Color) -> Self {
        Self { position, color }
    }
}

/// E6 gradient background: 2..=16 stops with non-decreasing positions in 0..=1.
/// E6 渐变背景：2..=16 个色标，位置在 0..=1 内且不减。
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub shape: GradientShape,
    pub stops: Vec<GradientStop>,
}

impl Gradient {
    pub fn linear(angle: f32, stops: Vec<GradientStop>) -> Self {
        Self {
            shape: GradientShape::Linear { angle },
            stops,
        }
    }

    pub fn radial(center_x: f32, center_y: f32, stops: Vec<GradientStop>) -> Self {
        Self {
            shape: GradientShape::Radial {
                center_x,
                center_y,
                radius: None,
            },
            stops,
        }
    }

    pub fn circle(center_x: f32, center_y: f32, radius: f32, stops: Vec<GradientStop>) -> Self {
        Self {
            shape: GradientShape::Radial {
                center_x,
                center_y,
                radius: Some(radius),
            },
            stops,
        }
    }

    pub fn valid(&self) -> bool {
        let geometry = match self.shape {
            GradientShape::Linear { angle } => {
                angle.is_finite() && angle.abs() <= MAX_TRANSFORM_ROTATE
            }
            GradientShape::Radial {
                center_x,
                center_y,
                radius,
            } => {
                [center_x, center_y]
                    .into_iter()
                    .all(|v| v.is_finite() && v.abs() <= MAX_TRANSFORM_ORIGIN)
                    && radius.is_none_or(|r| r.is_finite() && r > 0.0 && r <= MAX_EFFECT_EXTENT)
            }
        };
        geometry
            && (2..=MAX_GRADIENT_STOPS).contains(&self.stops.len())
            && self
                .stops
                .iter()
                .all(|s| s.position.is_finite() && (0.0..=1.0).contains(&s.position))
            && self
                .stops
                .windows(2)
                .all(|w| w[0].position <= w[1].position)
    }
}

/// Largest E5 blur radius (Gaussian standard deviation, logical px).
/// E5 模糊半径（高斯标准差，逻辑像素）的上界。
pub const MAX_BACKDROP_BLUR: f32 = 128.0;
/// Largest E5 saturation factor.
/// E5 饱和度系数的上界。
pub const MAX_BACKDROP_SATURATION: f32 = 8.0;

/// E5 material: the content painted behind the node is blurred (Gaussian, standard deviation
/// `blur` logical px, CSS `backdrop-filter: blur()`), saturated (CSS `saturate()`, 1 unchanged,
/// 0 grey), then `tint` is composited over it; an optional inner `stroke` follows the rounded
/// border box. When the engine cannot blur within its budget it paints `fallback` (or, without
/// one, `tint`) as a plain fill instead and reports the node (`backdrop_report`).
/// E5 材质：节点背后已绘内容经高斯模糊（标准差 `blur` 逻辑像素，同 CSS `backdrop-filter: blur()`）、
/// 饱和度（同 CSS `saturate()`，1 不变、0 灰度），再叠加 `tint`；可选内描边沿圆角边框盒。引擎在预算内无法模糊时
/// 改画 `fallback`（没有则画 `tint`）实底，并在 `backdrop_report` 中报告该节点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub blur: f32,
    pub saturation: f32,
    pub tint: Color,
    pub stroke: Option<BorderSide>,
    pub fallback: Option<Color>,
}

impl Backdrop {
    /// Blur and tint with saturation 1, no stroke and no fallback.
    /// 模糊与着色；饱和度 1，无描边、无回退色。
    pub const fn new(blur: f32, tint: Color) -> Self {
        Self {
            blur,
            saturation: 1.0,
            tint,
            stroke: None,
            fallback: None,
        }
    }

    pub const fn saturate(mut self, saturation: f32) -> Self {
        self.saturation = saturation;
        self
    }

    pub const fn stroke(mut self, width: f32, color: Color) -> Self {
        self.stroke = Some(BorderSide { width, color });
        self
    }

    pub const fn fallback(mut self, color: Color) -> Self {
        self.fallback = Some(color);
        self
    }

    pub fn valid(&self) -> bool {
        self.blur.is_finite()
            && (0.0..=MAX_BACKDROP_BLUR).contains(&self.blur)
            && self.saturation.is_finite()
            && (0.0..=MAX_BACKDROP_SATURATION).contains(&self.saturation)
            && self.stroke.is_none_or(|s| {
                s.width.is_finite() && s.width > 0.0 && s.width <= MAX_EFFECT_EXTENT
            })
    }

    /// Colour painted instead of the material when it is degraded.
    /// 材质降级时改画的颜色。
    pub fn fallback_color(&self) -> Color {
        self.fallback.unwrap_or(self.tint)
    }
}

/// Longest image resource id, in bytes.
/// 图片资源 id 的最大字节数。
pub const MAX_IMAGE_ID_BYTES: usize = 128;

/// A resource id is 1..=`MAX_IMAGE_ID_BYTES` bytes of ASCII letters, digits and `. _ - : / @ +`:
/// no whitespace, quotes, parentheses, commas or semicolons, so it is carried verbatim in an
/// `xstyle` value and in a JUI 2 field without escaping.
/// 资源 id 为 1..=`MAX_IMAGE_ID_BYTES` 字节的 ASCII 字母、数字与 `. _ - : / @ +`：不含空白、引号、括号、
/// 逗号与分号，因此可原样放入 `xstyle` 取值与 JUI 2 字段，无需转义。
pub fn image_id_valid(id: &str) -> bool {
    (1..=MAX_IMAGE_ID_BYTES).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':' | b'/' | b'@' | b'+'))
}

/// How the image is fitted into the content box (CSS `object-fit`).
/// 图片如何适配内容盒（同 CSS `object-fit`）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ObjectFit {
    /// Stretch to the box, ignoring the aspect ratio (the CSS initial value).
    /// 拉伸铺满内容盒，不保持宽高比（CSS 初始值）。
    #[default]
    Fill,
    /// Largest size that fits inside the box, aspect ratio kept (letterboxed).
    /// 保持宽高比、完整放入内容盒的最大尺寸（留边）。
    Contain,
    /// Smallest size that covers the box, aspect ratio kept (cropped).
    /// 保持宽高比、铺满内容盒的最小尺寸（裁切）。
    Cover,
    /// Natural size (logical px), cropped by the box.
    /// 自然尺寸（逻辑像素），超出内容盒部分裁掉。
    None,
    /// `None` when the natural size fits, otherwise `Contain`.
    /// 自然尺寸放得下时同 `None`，否则同 `Contain`。
    ScaleDown,
}

impl ObjectFit {
    /// The `xstyle` keyword (CSS spelling).
    /// `xstyle` 关键字（与 CSS 拼写相同）。
    pub const fn keyword(self) -> &'static str {
        match self {
            ObjectFit::Fill => "fill",
            ObjectFit::Contain => "contain",
            ObjectFit::Cover => "cover",
            ObjectFit::None => "none",
            ObjectFit::ScaleDown => "scale-down",
        }
    }

    pub fn from_keyword(text: &str) -> Option<Self> {
        Some(match text {
            "fill" => ObjectFit::Fill,
            "contain" => ObjectFit::Contain,
            "cover" => ObjectFit::Cover,
            "none" => ObjectFit::None,
            "scale-down" => ObjectFit::ScaleDown,
            _ => return None,
        })
    }
}

/// P0-A image content of a node. `source` names a resource the host injected into the view
/// (`SceneHostView::insert_image`); the engine never fetches anything itself. `align_x/align_y`
/// place the fitted image in the box (CSS `object-position` as fractions: 0 start, 0.5 centre,
/// 1 end). `tint`, when set, recolours every painted pixel to the tint colour keeping the image's
/// coverage (alpha × tint alpha): monochrome icons. SVG `currentColor` is the node's `color`.
/// 节点的 P0-A 图片内容。`source` 指向宿主注入本视图的资源（`SceneHostView::insert_image`），引擎自身从不获取任何资源。
/// `align_x/align_y` 决定适配后的图片在盒内的位置（CSS `object-position` 的比例形式：0 起始、0.5 居中、1 末端）。
/// `tint` 设置时把每个已绘像素改为着色色并保留图片覆盖度（alpha × 着色 alpha），用于单色图标。SVG 的
/// `currentColor` 取节点的 `color`。
#[derive(Clone, Debug, PartialEq)]
pub struct ImageStyle {
    pub source: String,
    pub fit: ObjectFit,
    pub align_x: f32,
    pub align_y: f32,
    pub tint: Option<Color>,
}

impl ImageStyle {
    /// `fill`, centred, untinted.
    /// `fill`、居中、不着色。
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            fit: ObjectFit::Fill,
            align_x: 0.5,
            align_y: 0.5,
            tint: None,
        }
    }

    pub fn fit(mut self, fit: ObjectFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn align(mut self, x: f32, y: f32) -> Self {
        self.align_x = x;
        self.align_y = y;
        self
    }

    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    /// Valid id and alignment fractions in 0..=1.
    /// id 合法且对齐比例在 0..=1。
    pub fn valid(&self) -> bool {
        image_id_valid(&self.source)
            && [self.align_x, self.align_y]
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }
}

/// One extended style capability as declared to the runtime (JUI 2 `hello` token) and the
/// `xstyle` keys it covers. Names follow the Jasper theme degradation matrix.
/// 向运行时声明的一项扩展样式能力（JUI 2 `hello` 令牌）及其覆盖的 `xstyle` 键；名称沿用 Jasper 主题降级矩阵。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleCapability {
    pub name: &'static str,
    pub xstyle_keys: &'static [&'static str],
}

/// E2/E3/E4/E7/E8/E9/E10 capabilities implemented by the native engine (`native-blitz-pending`).
/// The legacy prototype engine ignores these fields and declares none of them.
/// 原生引擎（`native-blitz-pending`）实现的 E2/E3/E4/E7/E8/E9/E10 能力；旧原型引擎忽略这些字段，不声明。
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
    StyleCapability {
        name: "transform",
        xstyle_keys: &["transform"],
    },
    StyleCapability {
        name: "transition",
        xstyle_keys: &["transition"],
    },
    StyleCapability {
        name: "material",
        xstyle_keys: &["backdrop"],
    },
    StyleCapability {
        name: "gradient",
        xstyle_keys: &["background-gradient"],
    },
    StyleCapability {
        name: "text-valign",
        xstyle_keys: &["text-valign", "line-height"],
    },
    // P0-A: bitmap images (PNG/JPEG/WebP) through the `image` key; requires a render feature.
    // P0-A：经 `image` 键的位图（PNG/JPEG/WebP）；需要渲染特性。
    StyleCapability {
        name: "image",
        xstyle_keys: &["image"],
    },
    // P0-A: SVG resources (rasterised per device pixel size, `currentColor`, tint) for the same
    // `image` key. A format capability: it adds no key of its own.
    // P0-A：同一 `image` 键可引用 SVG 资源（按设备像素尺寸光栅化、`currentColor`、着色）。这是格式能力，不另加键。
    StyleCapability {
        name: "svg",
        xstyle_keys: &[],
    },
    StyleCapability {
        name: "overlay",
        xstyle_keys: &["overlay"],
    },
    StyleCapability {
        name: "position",
        xstyle_keys: &["position", "inset", "z-index", "order"],
    },
    StyleCapability {
        name: "grid",
        xstyle_keys: &[
            "grid-template-columns",
            "grid-template-rows",
            "grid-column",
            "grid-row",
        ],
    },
];

/// `xstyle` key/value rejection.
/// `xstyle` 键值拒绝原因。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XStyleError {
    UnknownKey(String),
    InvalidValue { key: String, value: String },
}

impl core::fmt::Display for XStyleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            XStyleError::UnknownKey(key) => write!(f, "unknown xstyle key `{key}`"),
            XStyleError::InvalidValue { key, value } => {
                write!(f, "invalid xstyle value `{key}={value}`")
            }
        }
    }
}

impl core::error::Error for XStyleError {}

pub(crate) fn parse_number(text: &str) -> Option<f32> {
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

fn parse_length(text: &str) -> Option<Length> {
    match text {
        "auto" => Some(Length::Auto),
        _ if text.ends_with("px") => Some(Length::Px(parse_number(
            text.strip_suffix("px")?,
        )?)),
        _ if text.ends_with('%') => Some(Length::Percent(parse_number(
            text.strip_suffix('%')?,
        )?)),
        _ => None,
    }
}

fn format_length(length: Length) -> String {
    match length {
        Length::Auto => "auto".to_owned(),
        Length::Px(v) => format!("{}px", format_number(v)),
        Length::Percent(v) => format!("{}%", format_number(v)),
    }
}

fn expand_four<T: Copy>(values: &[T]) -> Option<[T; 4]> {
    match values {
        [a] => Some([*a; 4]),
        [a, b] => Some([*a, *b, *a, *b]),
        [a, b, c] => Some([*a, *b, *c, *b]),
        [a, b, c, d] => Some([*a, *b, *c, *d]),
        _ => None,
    }
}

fn parse_grid_track(text: &str) -> Option<GridTrack> {
    match text {
        "auto" => Some(GridTrack::Auto),
        _ if text.ends_with("fr") => {
            let value = parse_number(text.strip_suffix("fr")?)?;
            (value > 0.0).then_some(GridTrack::Fr(value))
        }
        _ if text.ends_with('%') => Some(GridTrack::Percent(parse_number(
            text.strip_suffix('%')?,
        )?)),
        _ => Some(GridTrack::Px(parse_number(
            text.strip_suffix("px").unwrap_or(text),
        )?)),
    }
}

fn format_grid_tracks(tracks: &[GridTrack]) -> String {
    tracks
        .iter()
        .map(|track| match track {
            GridTrack::Auto => "auto".to_owned(),
            GridTrack::Px(v) => format!("{}px", format_number(*v)),
            GridTrack::Percent(v) => format!("{}%", format_number(*v)),
            GridTrack::Fr(v) => format!("{}fr", format_number(*v)),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_grid_line(text: &str) -> Option<GridLine> {
    match text {
        "auto" => Some(GridLine::Auto),
        _ if text.starts_with("span ") => {
            let value = text.strip_prefix("span ")?.parse().ok()?;
            (value > 0).then_some(GridLine::Span(value))
        }
        _ => {
            let value: i32 = text.parse().ok()?;
            (value != 0).then_some(GridLine::Line(value))
        }
    }
}

fn format_grid_line(line: GridLine) -> String {
    match line {
        GridLine::Auto => "auto".to_owned(),
        GridLine::Line(v) => v.to_string(),
        GridLine::Span(v) => format!("span {v}"),
    }
}

pub(crate) fn parse_color(text: &str) -> Option<Color> {
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

pub(crate) fn format_color(c: Color) -> String {
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

pub(crate) fn format_number(v: f32) -> String {
    format!("{v}")
}

/// `<n>ms`, a plain decimal followed by the `ms` unit.
fn parse_ms(text: &str) -> Option<f32> {
    parse_number(text.trim().strip_suffix("ms")?)
}

/// `name(a,b,...)` → (name, args).
fn parse_call(text: &str) -> Option<(&str, Vec<f32>)> {
    let text = text.trim();
    let open = text.find('(')?;
    let args = text[open + 1..].strip_suffix(')')?;
    let values = args
        .split(',')
        .map(parse_number)
        .collect::<Option<Vec<f32>>>()?;
    Some((text[..open].trim(), values))
}

fn parse_easing(text: &str) -> Option<Easing> {
    Some(match text.trim() {
        "linear" => Easing::LINEAR,
        "ease" => Easing::EASE,
        "ease-in" => Easing::EASE_IN,
        "ease-out" => Easing::EASE_OUT,
        "ease-in-out" => Easing::EASE_IN_OUT,
        other => match parse_call(other)? {
            ("cubic", v) if v.len() == 4 => Easing::cubic(v[0], v[1], v[2], v[3]),
            ("spring", v) if v.len() == 3 => Easing::spring(v[0], v[1], v[2]),
            _ => return None,
        },
    })
}

fn format_easing(easing: Easing) -> String {
    match easing {
        Easing::CubicBezier { x1, y1, x2, y2 } => format!(
            "cubic({},{},{},{})",
            format_number(x1),
            format_number(y1),
            format_number(x2),
            format_number(y2)
        ),
        Easing::Spring {
            mass,
            stiffness,
            damping,
        } => format!(
            "spring({},{},{})",
            format_number(mass),
            format_number(stiffness),
            format_number(damping)
        ),
    }
}

fn parse_properties(text: &str) -> Option<AnimatedProperties> {
    let mut set = AnimatedProperties::NONE;
    for name in text.split(',').map(str::trim) {
        let item = match name {
            "all" => AnimatedProperties::ALL,
            "size" => AnimatedProperties::WIDTH.union(AnimatedProperties::HEIGHT),
            _ => {
                AnimatedProperties::NAMED
                    .iter()
                    .find(|(n, _)| *n == name)?
                    .1
            }
        };
        set = set.union(item);
    }
    (!set.is_empty()).then_some(set)
}

fn format_properties(set: AnimatedProperties) -> String {
    if set == AnimatedProperties::ALL {
        return "all".to_owned();
    }
    AnimatedProperties::NAMED
        .iter()
        .filter(|(_, p)| set.contains(*p))
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(",")
}

/// `<props>:<n>ms[:<easing>]` or `<props>:spring(m,k,c)`, then optional `delay=<n>ms` and
/// `reduce=<n>ms[/<easing>]` fields, all separated by `:`.
fn parse_transition(text: &str) -> Option<Transition> {
    let fields: Vec<&str> = text.split(':').map(str::trim).collect();
    let [props, timing, rest @ ..] = fields.as_slice() else {
        return None;
    };
    let properties = parse_properties(props)?;
    let (mut rule, mut rest) = if let Some(duration) = parse_ms(timing) {
        // An easing may follow the duration directly (default `ease`).
        match rest.split_first() {
            Some((easing, tail)) if !easing.contains('=') => match parse_easing(easing)? {
                bezier @ Easing::CubicBezier { .. } => {
                    (Transition::new(properties, duration, bezier), tail)
                }
                Easing::Spring { .. } => return None,
            },
            _ => (Transition::new(properties, duration, Easing::EASE), rest),
        }
    } else {
        match parse_easing(timing)? {
            spring @ Easing::Spring { .. } => (Transition::new(properties, 0.0, spring), rest),
            _ => return None,
        }
    };
    let (mut seen_delay, mut seen_reduce) = (false, false);
    while let Some((field, tail)) = rest.split_first() {
        rest = tail;
        if let Some(delay) = field.strip_prefix("delay=") {
            if core::mem::replace(&mut seen_delay, true) {
                return None;
            }
            rule.delay_ms = parse_ms(delay)?;
        } else if let Some(reduce) = field.strip_prefix("reduce=") {
            if core::mem::replace(&mut seen_reduce, true) {
                return None;
            }
            let (duration, easing) = match reduce.split_once('/') {
                Some((duration, easing)) => (parse_ms(duration)?, parse_easing(easing)?),
                None => (parse_ms(reduce)?, Easing::LINEAR),
            };
            rule.reduced = Some(ReducedTransition {
                duration_ms: duration,
                easing,
            });
        } else {
            return None;
        }
    }
    Some(rule)
}

fn format_transition(rule: &Transition) -> String {
    let mut text = format_properties(rule.properties);
    match rule.easing {
        Easing::Spring { .. } => {
            text.push(':');
            text.push_str(&format_easing(rule.easing));
        }
        Easing::CubicBezier { .. } => {
            text.push_str(&format!(
                ":{}ms:{}",
                format_number(rule.duration_ms),
                format_easing(rule.easing)
            ));
        }
    }
    if rule.delay_ms != 0.0 {
        text.push_str(&format!(":delay={}ms", format_number(rule.delay_ms)));
    }
    if let Some(r) = rule.reduced {
        text.push_str(&format!(
            ":reduce={}ms/{}",
            format_number(r.duration_ms),
            format_easing(r.easing)
        ));
    }
    text
}

/// `none`, or `translate(x,y)`, `rotate(deg)`, `scale(s)` / `scale(sx,sy)`, `origin(fx,fy)`,
/// each at most once, separated by `;` or spaces. Composition order is always CSS
/// translate → rotate → scale about the origin, whatever the written order.
fn parse_transform(text: &str) -> Option<Transform2D> {
    let text = text.trim();
    if text == "none" {
        return Some(Transform2D::IDENTITY);
    }
    let mut out = Transform2D::IDENTITY;
    let mut seen = [false; 4];
    let mut any = false;
    for part in text
        .split(|c: char| c == ';' || c.is_ascii_whitespace())
        .filter(|p| !p.is_empty())
    {
        any = true;
        let (name, v) = parse_call(part)?;
        let slot = match (name, v.as_slice()) {
            ("translate", [x, y]) => {
                (out.translate_x, out.translate_y) = (*x, *y);
                0
            }
            ("rotate", [r]) => {
                out.rotate = *r;
                1
            }
            ("scale", [s]) => {
                (out.scale_x, out.scale_y) = (*s, *s);
                2
            }
            ("scale", [x, y]) => {
                (out.scale_x, out.scale_y) = (*x, *y);
                2
            }
            ("origin", [x, y]) => {
                (out.origin_x, out.origin_y) = (*x, *y);
                3
            }
            _ => return None,
        };
        if core::mem::replace(&mut seen[slot], true) {
            return None;
        }
    }
    any.then_some(out)
}

fn format_transform(t: &Transform2D) -> String {
    let mut parts = Vec::new();
    if t.translate_x != 0.0 || t.translate_y != 0.0 {
        parts.push(format!(
            "translate({},{})",
            format_number(t.translate_x),
            format_number(t.translate_y)
        ));
    }
    if t.rotate != 0.0 {
        parts.push(format!("rotate({})", format_number(t.rotate)));
    }
    if t.scale_x != 1.0 || t.scale_y != 1.0 {
        parts.push(if t.scale_x == t.scale_y {
            format!("scale({})", format_number(t.scale_x))
        } else {
            format!(
                "scale({},{})",
                format_number(t.scale_x),
                format_number(t.scale_y)
            )
        });
    }
    if t.origin_x != 0.5 || t.origin_y != 0.5 {
        parts.push(format!(
            "origin({},{})",
            format_number(t.origin_x),
            format_number(t.origin_y)
        ));
    }
    if parts.is_empty() {
        "none".to_owned()
    } else {
        parts.join(";")
    }
}

/// `linear(<deg>,<stops>)`, `radial(<cx>,<cy>,<stops>)` or `radial(<cx>,<cy>,<r>,<stops>)`;
/// each stop is `#rrggbb[aa] <p>%`. Stops start at the first argument beginning with `#`.
fn parse_gradient(text: &str) -> Option<Gradient> {
    let text = text.trim();
    let open = text.find('(')?;
    let name = text[..open].trim();
    let args: Vec<&str> = text[open + 1..]
        .strip_suffix(')')?
        .split(',')
        .map(str::trim)
        .collect();
    let first_stop = args.iter().position(|a| a.starts_with('#'))?;
    let (geometry, stops) = args.split_at(first_stop);
    let geometry = geometry
        .iter()
        .map(|g| parse_number(g))
        .collect::<Option<Vec<f32>>>()?;
    let stops = stops
        .iter()
        .map(|stop| {
            let (color, position) = stop.split_once(char::is_whitespace)?;
            let percent = parse_number(position.trim().strip_suffix('%')?)?;
            Some(GradientStop::new(percent / 100.0, parse_color(color)?))
        })
        .collect::<Option<Vec<_>>>()?;
    let shape = match (name, geometry.as_slice()) {
        ("linear", [angle]) => GradientShape::Linear { angle: *angle },
        ("radial", [x, y]) => GradientShape::Radial {
            center_x: *x,
            center_y: *y,
            radius: None,
        },
        ("radial", [x, y, r]) => GradientShape::Radial {
            center_x: *x,
            center_y: *y,
            radius: Some(*r),
        },
        _ => return None,
    };
    Some(Gradient { shape, stops })
}

fn format_gradient(g: &Gradient) -> String {
    let mut args = match g.shape {
        GradientShape::Linear { angle } => vec![format_number(angle)],
        GradientShape::Radial {
            center_x,
            center_y,
            radius,
        } => {
            let mut v = vec![format_number(center_x), format_number(center_y)];
            if let Some(r) = radius {
                v.push(format_number(r));
            }
            v
        }
    };
    args.extend(
        g.stops
            .iter()
            .map(|s| format!("{} {}%", format_color(s.color), format_number(s.position * 100.0))),
    );
    format!(
        "{}({})",
        match g.shape {
            GradientShape::Linear { .. } => "linear",
            GradientShape::Radial { .. } => "radial",
        },
        args.join(",")
    )
}

/// `none`, or `blur(<σ>)`, `saturate(<s>)`, `tint(<colour>)`, `stroke(<w>,<colour>)`,
/// `fallback(<colour>)`, each at most once, separated by `;` or spaces (at least one).
fn parse_backdrop(text: &str) -> Option<Option<Backdrop>> {
    let text = text.trim();
    if text == "none" {
        return Some(None);
    }
    let mut out = Backdrop::new(0.0, Color::TRANSPARENT);
    let mut seen = [false; 5];
    let mut any = false;
    for part in text
        .split(|c: char| c == ';' || c.is_ascii_whitespace())
        .filter(|p| !p.is_empty())
    {
        any = true;
        let open = part.find('(')?;
        let name = &part[..open];
        let args: Vec<&str> = part[open + 1..].strip_suffix(')')?.split(',').collect();
        let slot = match (name, args.as_slice()) {
            ("blur", [v]) => {
                out.blur = parse_number(v)?;
                0
            }
            ("saturate", [v]) => {
                out.saturation = parse_number(v)?;
                1
            }
            ("tint", [c]) => {
                out.tint = parse_color(c)?;
                2
            }
            ("stroke", [w, c]) => {
                out.stroke = Some(BorderSide {
                    width: parse_number(w)?,
                    color: parse_color(c)?,
                });
                3
            }
            ("fallback", [c]) => {
                out.fallback = Some(parse_color(c)?);
                4
            }
            _ => return None,
        };
        if core::mem::replace(&mut seen[slot], true) {
            return None;
        }
    }
    any.then_some(Some(out))
}

fn format_backdrop(b: &Backdrop) -> String {
    let mut parts = vec![
        format!("blur({})", format_number(b.blur)),
        format!("saturate({})", format_number(b.saturation)),
        format!("tint({})", format_color(b.tint)),
    ];
    if let Some(s) = b.stroke {
        parts.push(format!(
            "stroke({},{})",
            format_number(s.width),
            format_color(s.color)
        ));
    }
    if let Some(c) = b.fallback {
        parts.push(format!("fallback({})", format_color(c)));
    }
    parts.join(";")
}

/// `none`, or `src(<id>)` plus optional `fit(<keyword>)`, `align(<x>,<y>)`, `tint(<colour>)`,
/// each at most once, separated by `;` or whitespace.
/// `none`，或 `src(<id>)` 加可选的 `fit(<关键字>)`、`align(<x>,<y>)`、`tint(<颜色>)`，各至多一次，以 `;` 或空白分隔。
fn parse_image(text: &str) -> Option<Option<ImageStyle>> {
    let text = text.trim();
    if text == "none" {
        return Some(None);
    }
    let mut source = None;
    let mut out = ImageStyle::new(String::new());
    let mut seen = [false; 4];
    for part in text
        .split(|c: char| c == ';' || c.is_ascii_whitespace())
        .filter(|p| !p.is_empty())
    {
        let open = part.find('(')?;
        let name = &part[..open];
        let args: Vec<&str> = part[open + 1..].strip_suffix(')')?.split(',').collect();
        let slot = match (name, args.as_slice()) {
            ("src", [id]) if image_id_valid(id) => {
                source = Some((*id).to_owned());
                0
            }
            ("fit", [k]) => {
                out.fit = ObjectFit::from_keyword(k)?;
                1
            }
            ("align", [x, y]) => {
                out.align_x = parse_number(x)?;
                out.align_y = parse_number(y)?;
                2
            }
            ("tint", [c]) => {
                out.tint = Some(parse_color(c)?);
                3
            }
            _ => return None,
        };
        if core::mem::replace(&mut seen[slot], true) {
            return None;
        }
    }
    out.source = source?;
    Some(Some(out))
}

fn format_image(i: &ImageStyle) -> String {
    let mut parts = vec![
        format!("src({})", i.source),
        format!("fit({})", i.fit.keyword()),
        format!(
            "align({},{})",
            format_number(i.align_x),
            format_number(i.align_y)
        ),
    ];
    if let Some(c) = i.tint {
        parts.push(format!("tint({})", format_color(c)));
    }
    parts.join(";")
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
                "transform" => {
                    next.transform = parse_transform(value_trimmed).ok_or_else(invalid)?;
                }
                "transition" => {
                    next.transitions = if value_trimmed == "none" {
                        Vec::new()
                    } else {
                        value_trimmed
                            .split(';')
                            .map(parse_transition)
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(invalid)?
                    };
                }
                "background-gradient" => {
                    next.background_gradient = if value_trimmed == "none" {
                        None
                    } else {
                        Some(parse_gradient(value_trimmed).ok_or_else(invalid)?)
                    };
                }
                "backdrop" => {
                    next.backdrop = parse_backdrop(value_trimmed).ok_or_else(invalid)?;
                }
                "image" => {
                    next.image = parse_image(value_trimmed).ok_or_else(invalid)?;
                }
                "text-valign" => {
                    next.text_align_vertical = match value_trimmed {
                        "none" => None,
                        "top" => Some(TextAlignVertical::Top),
                        "center" => Some(TextAlignVertical::Center),
                        "bottom" => Some(TextAlignVertical::Bottom),
                        _ => return Err(invalid()),
                    };
                }
                "line-height" => {
                    next.line_height = if value_trimmed == "none" {
                        None
                    } else if let Some(px) = value_trimmed.strip_suffix("px") {
                        Some(LineHeight::Px(parse_number(px).ok_or_else(invalid)?))
                    } else {
                        Some(LineHeight::Multiple(
                            parse_number(value_trimmed).ok_or_else(invalid)?,
                        ))
                    };
                }
                "overlay" => {
                    next.overlay = crate::overlay::parse_overlay(value_trimmed).ok_or_else(invalid)?;
                }
                "position" => {
                    next.position = match value_trimmed {
                        "static" => Position::Static,
                        "relative" => Position::Relative,
                        "absolute" => Position::Absolute,
                        "fixed" => Position::Fixed,
                        "sticky" => Position::Sticky,
                        _ => return Err(invalid()),
                    };
                }
                "inset" => {
                    let values = value_trimmed
                        .split(',')
                        .map(|v| parse_length(v.trim()).ok_or_else(invalid))
                        .collect::<Result<Vec<_>, _>>()?;
                    let [top, right, bottom, left] = expand_four(&values).ok_or_else(invalid)?;
                    next.inset = Inset::new(top, right, bottom, left);
                }
                "z-index" => {
                    next.z_index = if value_trimmed == "auto" {
                        None
                    } else {
                        Some(value_trimmed.parse().map_err(|_| invalid())?)
                    };
                }
                "order" => {
                    next.order = value_trimmed.parse().map_err(|_| invalid())?;
                }
                "grid-template-columns" | "grid-template-rows" => {
                    let tracks = if value_trimmed == "none" {
                        None
                    } else {
                        Some(
                            value_trimmed
                                .split_ascii_whitespace()
                                .map(|v| parse_grid_track(v).ok_or_else(invalid))
                                .collect::<Result<Vec<_>, _>>()?,
                        )
                    };
                    if key == "grid-template-columns" {
                        next.grid_columns = tracks;
                    } else {
                        next.grid_rows = tracks;
                    }
                }
                "grid-column" => next.grid_column = parse_grid_line(value_trimmed).ok_or_else(invalid)?,
                "grid-row" => next.grid_row = parse_grid_line(value_trimmed).ok_or_else(invalid)?,
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
        if self.transform != Transform2D::IDENTITY {
            pairs.push(("transform", format_transform(&self.transform)));
        }
        if !self.transitions.is_empty() {
            pairs.push((
                "transition",
                self.transitions
                    .iter()
                    .map(format_transition)
                    .collect::<Vec<_>>()
                    .join(";"),
            ));
        }
        if let Some(g) = &self.background_gradient {
            pairs.push(("background-gradient", format_gradient(g)));
        }
        if let Some(b) = &self.backdrop {
            pairs.push(("backdrop", format_backdrop(b)));
        }
        if let Some(i) = &self.image {
            pairs.push(("image", format_image(i)));
        }
        if self.position != Position::Static {
            pairs.push((
                "position",
                match self.position {
                    Position::Static => "static",
                    Position::Relative => "relative",
                    Position::Absolute => "absolute",
                    Position::Fixed => "fixed",
                    Position::Sticky => "sticky",
                }
                .to_owned(),
            ));
        }
        let i = self.inset;
        if i != Inset::ZERO {
            pairs.push((
                "inset",
                [i.top, i.right, i.bottom, i.left]
                    .map(format_length)
                    .join(","),
            ));
        }
        if let Some(z) = self.z_index {
            pairs.push(("z-index", z.to_string()));
        }
        if self.order != 0 {
            pairs.push(("order", self.order.to_string()));
        }
        if let Some(tracks) = &self.grid_columns {
            pairs.push(("grid-template-columns", format_grid_tracks(tracks)));
        }
        if let Some(tracks) = &self.grid_rows {
            pairs.push(("grid-template-rows", format_grid_tracks(tracks)));
        }
        if self.grid_column != GridLine::Auto {
            pairs.push(("grid-column", format_grid_line(self.grid_column)));
        }
        if self.grid_row != GridLine::Auto {
            pairs.push(("grid-row", format_grid_line(self.grid_row)));
        }
        if let Some(a) = self.text_align_vertical {
            pairs.push((
                "text-valign",
                match a {
                    TextAlignVertical::Top => "top",
                    TextAlignVertical::Center => "center",
                    TextAlignVertical::Bottom => "bottom",
                }
                .to_owned(),
            ));
        }
        if let Some(h) = self.line_height {
            pairs.push((
                "line-height",
                match h {
                    LineHeight::Px(v) => format!("{}px", format_number(v)),
                    LineHeight::Multiple(v) => format_number(v),
                },
            ));
        }
        if let Some(o) = &self.overlay {
            pairs.push(("overlay", crate::overlay::format_overlay(o)));
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
        s.apply_xstyle("position", "absolute").unwrap();
        s.apply_xstyle("inset", "1px,2%,auto,4px").unwrap();
        s.apply_xstyle("z-index", "-3").unwrap();
        s.apply_xstyle("order", "7").unwrap();
        s.apply_xstyle("grid-template-columns", "40px 1fr 20% auto")
            .unwrap();
        s.apply_xstyle("grid-template-rows", "2fr 30px").unwrap();
        s.apply_xstyle("grid-column", "span 2").unwrap();
        s.apply_xstyle("grid-row", "-1").unwrap();
        assert_eq!(s.position, Position::Absolute);
        assert_eq!(
            s.inset,
            Inset::new(
                Length::Px(1.0),
                Length::Percent(2.0),
                Length::Auto,
                Length::Px(4.0)
            )
        );
        assert_eq!(s.z_index, Some(-3));
        assert_eq!(s.order, 7);
        assert_eq!(
            s.grid_columns,
            Some(vec![
                GridTrack::Px(40.0),
                GridTrack::Fr(1.0),
                GridTrack::Percent(20.0),
                GridTrack::Auto
            ])
        );
        assert_eq!(s.grid_column, GridLine::Span(2));
        assert_eq!(s.grid_row, GridLine::Line(-1));

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
                "position" => "static",
                "inset" => "auto",
                "z-index" => "auto",
                "order" => "0",
                "grid-column" | "grid-row" => "auto",
                _ => "none",
            };
            s.apply_xstyle(key, value).unwrap();
        }
        assert_eq!(s, Style::default());
    }

    #[test]
    fn xstyle_transform_and_transition() {
        let mut s = Style::default();
        s.apply_xstyle("transform", "translate(4,-2) rotate(15) scale(0.96)")
            .unwrap();
        assert_eq!(
            s.transform,
            Transform2D {
                translate_x: 4.0,
                translate_y: -2.0,
                rotate: 15.0,
                scale_x: 0.96,
                scale_y: 0.96,
                origin_x: 0.5,
                origin_y: 0.5,
            }
        );
        s.apply_xstyle("transform", "scale(2,0.5);origin(0,1)")
            .unwrap();
        assert_eq!(
            s.transform,
            Transform2D::IDENTITY
                .with_origin(0.0, 1.0)
                .scale_xy(2.0, 0.5)
        );
        s.apply_xstyle(
            "transition",
            "opacity,background:200ms:cubic(0.2,0,0,1):reduce=150ms;transform:spring(1,170,26):delay=50ms",
        )
        .unwrap();
        assert_eq!(
            s.transitions,
            vec![
                Transition::new(
                    AnimatedProperties::OPACITY.union(AnimatedProperties::BACKGROUND),
                    200.0,
                    Easing::cubic(0.2, 0.0, 0.0, 1.0)
                )
                .reduced(150.0, Easing::LINEAR),
                Transition::spring(AnimatedProperties::TRANSFORM, 1.0, 170.0, 26.0).delay(50.0),
            ]
        );
        assert!(s.transition_for(AnimatedProperties::TRANSFORM).is_some());
        assert!(s.transition_for(AnimatedProperties::WIDTH).is_none());
        // Keywords, default easing, `all`, `size` and the reduced easing.
        let mut t = Style::default();
        t.apply_xstyle(
            "transition",
            "all:120ms;size:80ms:ease-out:reduce=60ms/ease-in",
        )
        .unwrap();
        assert_eq!(t.transitions[0].properties, AnimatedProperties::ALL);
        assert_eq!(t.transitions[0].easing, Easing::EASE);
        assert_eq!(
            t.transitions[1],
            Transition::new(
                AnimatedProperties::WIDTH.union(AnimatedProperties::HEIGHT),
                80.0,
                Easing::EASE_OUT
            )
            .reduced(60.0, Easing::EASE_IN)
        );
        // Later rules win.
        assert_eq!(
            t.transition_for(AnimatedProperties::WIDTH),
            Some(&t.transitions[1])
        );
        assert_eq!(
            t.transition_for(AnimatedProperties::OPACITY),
            Some(&t.transitions[0])
        );
        // Round trip through the canonical pairs.
        for style in [&s, &t] {
            let mut copy = Style::default();
            for (key, value) in style.xstyle_pairs() {
                copy.apply_xstyle(key, &value).unwrap();
            }
            assert_eq!(&copy, style);
        }
        s.apply_xstyle("transform", "none").unwrap();
        s.apply_xstyle("transition", "none").unwrap();
        assert_eq!(s, Style::default());

        let before = Style::default().corner_radius(2.0);
        for (key, value) in [
            ("transform", ""),
            ("transform", "rotate(1deg)"),
            ("transform", "scale(1) scale(2)"),
            ("transform", "skew(3)"),
            ("transform", "scale(65)"),
            ("transform", "translate(1)"),
            ("transform", "origin(9,0)"),
            ("transition", ""),
            ("transition", "opacity"),
            ("transition", "opacity:200"),
            ("transition", "opacity:0.2s"),
            ("transition", "glow:200ms"),
            ("transition", "opacity:-1ms"),
            ("transition", "opacity:60001ms"),
            ("transition", "opacity:200ms:cubic(1.2,0,0,1)"),
            ("transition", "opacity:200ms:cubic(0,9,0,1)"),
            ("transition", "opacity:200ms:spring(1,170,26)"),
            ("transition", "opacity:spring(0,170,26)"),
            ("transition", "opacity:spring(1,170,0)"),
            ("transition", "opacity:spring(1,170)"),
            ("transition", "opacity:200ms:reduce=100ms/spring(1,1,1)"),
            ("transition", "opacity:200ms:delay=1ms:delay=2ms"),
            ("transition", "opacity:200ms:bounce"),
            ("transition", "opacity:200ms:linear:linear"),
        ] {
            let mut s = before.clone();
            assert!(
                matches!(
                    s.apply_xstyle(key, value),
                    Err(XStyleError::InvalidValue { .. })
                ),
                "{key}={value}"
            );
            assert_eq!(s, before);
        }
        let mut many = String::new();
        for i in 0..=MAX_TRANSITIONS {
            if i > 0 {
                many.push(';');
            }
            many.push_str("opacity:1ms");
        }
        assert!(Style::default().apply_xstyle("transition", &many).is_err());
    }

    #[test]
    fn transform_matrix_matches_css_order() {
        // rotate(90) about the centre of a 10×20 box at (100, 50): the centre is fixed and the
        // top-left corner maps to the bottom-left side of the rotated box.
        let m = Transform2D::rotate(90.0).matrix(100.0, 50.0, 10.0, 20.0);
        let apply = |x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let (cx, cy) = apply(105.0, 60.0);
        assert!((cx - 105.0).abs() < 1e-9 && (cy - 60.0).abs() < 1e-9);
        let (x, y) = apply(100.0, 50.0);
        assert!(
            (x - 115.0).abs() < 1e-9 && (y - 55.0).abs() < 1e-9,
            "({x},{y})"
        );
        // translate then scale about the top-left origin.
        let t = Transform2D {
            translate_x: 5.0,
            ..Transform2D::scale(2.0).with_origin(0.0, 0.0)
        };
        let m = t.matrix(10.0, 10.0, 4.0, 4.0);
        assert_eq!(m, [2.0, 0.0, 0.0, 2.0, -5.0, -10.0]);
        assert!(Transform2D::IDENTITY.is_identity());
        assert!(Transform2D::IDENTITY.with_origin(0.0, 0.0).is_identity());
        assert!(!Transform2D::scale(0.0).is_identity());
    }

    #[test]
    fn xstyle_gradient_and_backdrop() {
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let mut s = Style::default();
        s.apply_xstyle("background-gradient", "linear(90,#ff0000 0%,#0000ffff 100%)")
            .unwrap();
        assert_eq!(
            s.background_gradient,
            Some(Gradient::linear(
                90.0,
                vec![GradientStop::new(0.0, red), GradientStop::new(1.0, blue)]
            ))
        );
        s.apply_xstyle(
            "background-gradient",
            "radial(0.25,0.75,#ff0000 0%,#ff000000 40%,#0000ff 100%)",
        )
        .unwrap();
        let g = s.background_gradient.clone().unwrap();
        assert_eq!(
            g.shape,
            GradientShape::Radial {
                center_x: 0.25,
                center_y: 0.75,
                radius: None
            }
        );
        assert_eq!(g.stops.len(), 3);
        assert!((g.stops[1].position - 0.4).abs() < 1e-6);
        s.apply_xstyle("background-gradient", "radial(0.5,0.5,24,#ff0000 0%,#0000ff 100%)")
            .unwrap();
        assert!(matches!(
            s.background_gradient.as_ref().unwrap().shape,
            GradientShape::Radial {
                radius: Some(r),
                ..
            } if r == 24.0
        ));
        s.apply_xstyle(
            "backdrop",
            "blur(20) saturate(1.8) tint(#ffffff80) stroke(1,#ffffff33) fallback(#f2f2f7)",
        )
        .unwrap();
        assert_eq!(
            s.backdrop,
            Some(
                Backdrop::new(20.0, Color::new(255, 255, 255, 128))
                    .saturate(1.8)
                    .stroke(1.0, Color::new(255, 255, 255, 0x33))
                    .fallback(Color::rgb(0xf2, 0xf2, 0xf7))
            )
        );
        s.apply_xstyle("backdrop", "tint(#00000040)").unwrap();
        let b = s.backdrop.unwrap();
        assert_eq!((b.blur, b.saturation, b.stroke, b.fallback), (0.0, 1.0, None, None));
        assert_eq!(b.fallback_color(), Color::new(0, 0, 0, 0x40));

        // Canonical pairs reproduce the style.
        s.apply_xstyle("backdrop", "blur(12);saturate(1.5);tint(#11223344);fallback(#ffffff)")
            .unwrap();
        let mut copy = Style::default();
        for (k, v) in s.xstyle_pairs() {
            copy.apply_xstyle(k, &v).unwrap();
        }
        assert_eq!(copy, s);
        s.apply_xstyle("background-gradient", "none").unwrap();
        s.apply_xstyle("backdrop", "none").unwrap();
        assert_eq!(s, Style::default());

        // Rejections leave the style unchanged.
        let before = s.clone();
        for (k, v) in [
            ("background-gradient", "linear(90,#ff0000 0%)"),
            ("background-gradient", "linear(90,#ff0000 60%,#0000ff 40%)"),
            ("background-gradient", "linear(90,#ff0000 0%,#0000ff 120%)"),
            ("background-gradient", "linear(90,#ff0000,#0000ff)"),
            ("background-gradient", "linear(90deg,#ff0000 0%,#0000ff 100%)"),
            ("background-gradient", "conic(0,#ff0000 0%,#0000ff 100%)"),
            ("background-gradient", "radial(0.5,#ff0000 0%,#0000ff 100%)"),
            ("background-gradient", "radial(0.5,0.5,0,#ff0000 0%,#0000ff 100%)"),
            ("background-gradient", "radial(9,0.5,#ff0000 0%,#0000ff 100%)"),
            ("backdrop", ""),
            ("backdrop", "blur(200)"),
            ("backdrop", "blur(-1)"),
            ("backdrop", "saturate(9)"),
            ("backdrop", "blur(2) blur(3)"),
            ("backdrop", "tint(red)"),
            ("backdrop", "stroke(0,#ffffff)"),
            ("backdrop", "glass(1)"),
        ] {
            assert!(s.apply_xstyle(k, v).is_err(), "{k}={v}");
            assert_eq!(s, before);
        }
        let stops = (0..=MAX_GRADIENT_STOPS)
            .map(|i| format!("#000000 {}%", i))
            .collect::<Vec<_>>()
            .join(",");
        assert!(s
            .apply_xstyle("background-gradient", &format!("linear(0,{stops})"))
            .is_err());
        assert!(AnimatedProperties::ALL.contains(AnimatedProperties::BACKDROP));
        s.apply_xstyle("transition", "backdrop,gradient:200ms").unwrap();
        assert_eq!(
            s.transitions[0].properties,
            AnimatedProperties::BACKDROP.union(AnimatedProperties::GRADIENT)
        );
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
