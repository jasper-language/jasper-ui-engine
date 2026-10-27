//! Local synchronous text frames, NOT ACCEPTED: six-stage verification is pending.
//! 本地同步文本帧，尚未验收：自有Rust六阶段验证待完成。
//! Skia/font parsing/shaping are explicit unproved native dependencies.
//! Skia、字体解析与排版是显式未证明原生依赖；不保证jasper-os宿主适用性。
use crate::{
    backend::BackendError,
    geometry::{Rect, Size},
    layout::{LayoutEngine, LayoutTree, TaffyLayoutEngine},
    paint::{paint, PaintOp},
    scene::{Node, NodeId, Scene},
    style::Color,
    surface::{rgba8_row_bytes, surface_span},
    text::{TextEngine, TextMetrics},
};
use skia_safe::textlayout::{
    Affinity, FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextStyle, TypefaceFontProvider,
};
use skia_safe::{
    surfaces, AlphaType, ClipOp, ColorType, Data, FontMgr, ImageInfo, Paint, PaintStyle,
};

/// Explicit font input; native storage copies these bytes during construction.
/// 显式字体输入；构造时原生存储复制字节，不查询系统字体。
pub struct InjectedFont<'a> {
    pub bytes: &'a [u8],
    pub collection_index: Option<u32>,
}

/// Single-thread local font context; not a portable IPC rendering backend.
/// 单线程本地字体上下文，不作为可移植IPC渲染后端。
/// ```compile_fail,E0277
/// use jasper_ui_engine::text_skia::LocalTextRenderer;
/// fn require_send<T: Send>() {}
/// require_send::<LocalTextRenderer>();
/// ```
pub struct LocalTextRenderer {
    ui_context: std::marker::PhantomData<std::rc::Rc<()>>,
    fonts: FontCollection,
    families: Vec<String>,
}

/// An owned prepared frame keeps its source scene immutably borrowed until consumed.
/// 已准备帧持续不可变借用源场景，消费前不能通过安全Rust修改公开root。
/// No global cache or version-only identity admission is used.
/// 不使用全局缓存，也不把版本号单独当成身份准入。
/// ```compile_fail,E0502
/// use jasper_ui_engine::{text_skia::LocalTextRenderer,Scene,Color};
/// fn cannot_mutate(renderer:&LocalTextRenderer,scene:&mut Scene) {
///     let frame=renderer.prepare(scene,64,64,64*64*4,Color::WHITE).unwrap();
///     scene.root.children.clear();
///     drop(frame);
/// }
/// ```
/// ```compile_fail,E0277
/// use jasper_ui_engine::text_skia::LocalTextFrame;
/// fn require_sync<T: Sync>() {}
/// require_sync::<LocalTextFrame<'static>>();
/// ```
pub struct LocalTextFrame<'a> {
    ui_context: std::marker::PhantomData<std::rc::Rc<()>>,
    source: &'a Scene,
    layout: LayoutTree,
    ops: Vec<PaintOp>,
    paragraphs: Vec<PreparedText>,
    width: u32,
    height: u32,
    row_bytes: usize,
    byte_len: usize,
}

/// Tentative local geometry result; UTF-16 units, not UTF-8 bytes or an edit command.
/// 暂定本地几何命中结果；单位是UTF-16，不是UTF-8字节，也不直接授权编辑。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextHit {
    pub utf16_offset: usize,
    pub affinity: TextAffinity,
}
/// Visual affinity at a logical offset; independent of native Skia public types.
/// 逻辑位置的视觉亲和性，不在公共结果中暴露Skia私有类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAffinity { Upstream, Downstream }
struct PreparedText {
    node: NodeId,
    origin: crate::geometry::Point,
    utf16_len: usize,
    paragraph: Paragraph,
}

fn invalid(message: &str) -> BackendError {
    BackendError::Invalid(message.into())
}
fn native_color(c: Color) -> skia_safe::Color {
    skia_safe::Color::from_argb(c.a, c.r, c.g, c.b)
}
fn native_rect(r: Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(r.x(), r.y(), r.width(), r.height())
}
fn finite_rect(r: Rect) -> bool {
    [r.x(), r.y(), r.width(), r.height()]
        .iter()
        .all(|v| v.is_finite())
        && r.width() >= 0.0
        && r.height() >= 0.0
        && (r.x() + r.width()).is_finite()
        && (r.y() + r.height()).is_finite()
}
// Reject nonfinite native draw parameters before creating an output surface.
// 创建输出Surface前拒绝非有限原生绘制参数；不把NaN传入原生路径。
fn valid_draw_op(op: &PaintOp) -> bool {
    match op {
        PaintOp::Clear(_) | PaintOp::PopClip => true,
        PaintOp::FillRect { rect, .. } | PaintOp::PushClip(rect) => finite_rect(*rect),
        PaintOp::StrokeRect { rect, width, .. } => {
            finite_rect(*rect) && width.is_finite() && *width >= 0.0
        }
        PaintOp::Line {
            from, to, width, ..
        } => {
            [from.x, from.y, to.x, to.y, *width]
                .iter()
                .all(|v| v.is_finite())
                && *width >= 0.0
        }
        PaintOp::DrawText {
            origin, font_size, ..
        } => {
            origin.x.is_finite()
                && origin.y.is_finite()
                && font_size.is_finite()
                && *font_size > 0.0
        }
    }
}
fn valid_fonts(node: &Node) -> bool {
    node.style.font_size.is_finite()
        && node.style.font_size > 0.0
        && node.children.iter().all(valid_fonts)
}
// BEGIN VERIFIED TEXT HIT OFFSET KERNEL
/// Admit a native signed UTF-16 position only inside the prepared text extent.
/// 原生有符号UTF-16位置必须非负且位于已准备文字范围内；不等同字素边界证明。
fn text_hit_offset(position: i32, utf16_len: usize) -> Option<usize> {
    if position < 0 {
        return None;
    }
    let offset = position as usize;
    if offset > utf16_len {
        None
    } else {
        Some(offset)
    }
}
// END VERIFIED TEXT HIT OFFSET KERNEL

// BEGIN VERIFIED LOCAL TEXT STORAGE KERNEL
/// Numerical admission used by the actual local text frame allocator.
/// 真实本地文本帧分配入口使用的数值准入，不证明原生分配或绘制。
fn text_frame_storage(width: u32, height: u32, max_bytes: usize) -> Option<(usize, usize)> {
    if width > i32::MAX as u32 || height > i32::MAX as u32 {
        return None;
    }
    let row = match rgba8_row_bytes(width, max_bytes) {
        Some(value) => value,
        None => return None,
    };
    match surface_span(row, height as usize, row, max_bytes) {
        Some(end) => Some((row, end)),
        None => None,
    }
}
// END VERIFIED LOCAL TEXT STORAGE KERNEL

impl LocalTextRenderer {
    /// Inject every face and disable implicit host font fallback.
    /// 注入所有字体并禁用隐式宿主字体回退；解析失败不返回部分上下文。
    pub fn new(fonts: &[InjectedFont<'_>]) -> Result<Self, BackendError> {
        if fonts.is_empty() {
            return Err(invalid("explicit fonts required"));
        }
        let manager =
            FontMgr::custom_empty().ok_or_else(|| invalid("memory font manager unavailable"))?;
        let mut provider = TypefaceFontProvider::new();
        let mut families = Vec::new();
        for (i, font) in fonts.iter().enumerate() {
            let face = manager
                .new_from_data(Data::new_copy(font.bytes), font.collection_index)
                .ok_or_else(|| invalid("font parsing failed"))?;
            let alias = format!("JasperInjected{i}");
            provider.register_typeface(face, Some(alias.as_str()));
            families.push(alias);
        }
        let mut collection = FontCollection::new();
        let registered: FontMgr = provider.into();
        collection.set_asset_font_manager(registered);
        collection.disable_font_fallback();
        Ok(Self {
            ui_context: std::marker::PhantomData,
            fonts: collection,
            families,
        })
    }
    fn paragraph(&self, text: &str, size: f32, width: f32, color: Color) -> Paragraph {
        let mut ts = TextStyle::new();
        ts.set_font_families(&self.families);
        ts.set_font_size(size).set_color(native_color(color));
        let mut ps = ParagraphStyle::new();
        ps.set_text_style(&ts);
        let mut builder = ParagraphBuilder::new(&ps, self.fonts.clone());
        builder.add_text(text);
        let mut p = builder.build();
        p.layout(width);
        p
    }
    /// Prepare against final Taffy widths while retaining the same scene borrow.
    /// 按Taffy最终宽度准备段落，并保持同一场景借用；失败不发布帧。
    pub fn prepare<'a>(
        &self,
        scene: &'a Scene,
        width: u32,
        height: u32,
        max_bytes: usize,
        clear: Color,
    ) -> Result<LocalTextFrame<'a>, BackendError> {
        let (row_bytes, byte_len) = text_frame_storage(width, height, max_bytes)
            .ok_or_else(|| invalid("surface dimensions or allocation budget"))?;
        if !valid_fonts(&scene.root) {
            return Err(invalid("invalid font size"));
        }
        let layout = TaffyLayoutEngine
            .layout(scene, Size::new(width as f32, height as f32), self)
            .map_err(|e| invalid(&format!("layout: {e:?}")))?;
        let mut paragraphs = Vec::new();
        for node in &layout.boxes {
            if !finite_rect(node.content)
                || !finite_rect(node.rect)
                || node.clip.is_some_and(|r| !finite_rect(r))
            {
                return Err(invalid("invalid layout geometry"));
            }
            if let Some(text) = &node.text {
                if !text.is_empty() {
                    let mut p = self.paragraph(
                        text,
                        node.style.font_size,
                        node.content.width(),
                        node.style.color,
                    );
                    if p.unresolved_glyphs() != Some(0)
                        || !p.height().is_finite()
                        || (p.height() - node.content.height()).abs() > 0.1
                    {
                        return Err(invalid("unresolved glyphs or inconsistent text height"));
                    }
                    paragraphs.push(PreparedText {
                        node: node.node_id,
                        origin: node.content.origin,
                        utf16_len: text.encode_utf16().count(),
                        paragraph: p,
                    });
                }
            }
            // Canvas text has no final layout width in the existing protocol.
            // 现有Canvas文字协议缺最终布局宽度，不把它混入已准备段落队列。
            if node.canvas.as_ref().is_some_and(|c| {
                c.to_paint_ops(node.content.origin)
                    .iter()
                    .any(|op| matches!(op, PaintOp::DrawText { .. }))
            }) {
                return Err(BackendError::Unsupported("canvas text preparation"));
            }
        }
        let ops = paint(&layout, clear).ops;
        if !ops.iter().all(valid_draw_op) {
            return Err(invalid("invalid native draw parameters"));
        }
        Ok(LocalTextFrame {
            ui_context: std::marker::PhantomData,
            source: scene,
            layout,
            ops,
            paragraphs,
            width,
            height,
            row_bytes,
            byte_len,
        })
    }
}
impl TextEngine for LocalTextRenderer {
    fn measure(&self, text: &str, size: f32, width: Option<f32>) -> TextMetrics {
        // The legacy measurement trait cannot return an error; prepare validates inputs.
        // 旧测量接口无错误返回；prepare严格校验，本接口对非法参数返回空指标。
        if !size.is_finite() || size <= 0.0 || width.is_some_and(|w| !w.is_finite() || w < 0.0) {
            return TextMetrics::default();
        }
        let p = self.paragraph(text, size, width.unwrap_or(f32::MAX), Color::BLACK);
        TextMetrics {
            width: p.longest_line(),
            height: p.height(),
            ascent: p.alphabetic_baseline(),
            lines: u32::try_from(p.line_number()).unwrap_or(u32::MAX),
        }
    }
}
impl LocalTextFrame<'_> {
    pub fn layout(&self) -> &LayoutTree {
        &self.layout
    }
    pub fn scene(&self) -> &Scene {
        self.source
    }
    /// Query the same prepared Paragraph that will paint this node.
    /// 查询后续绘制同一节点使用的已准备Paragraph；调用者须先完成输入归属准入。
    /// Viewport coordinates are translated to paragraph-local coordinates. A finite
    /// outside point selects the nearest native text position, not another node.
    /// 视口坐标转为段落局部坐标；有限越界点仅取指定节点最近位置，不转交其他节点。
    pub fn hit_test_text(&self, node: NodeId, point: crate::geometry::Point) -> Option<TextHit> {
        if !point.x.is_finite() || !point.y.is_finite() { return None; }
        let prepared = self.paragraphs.iter().find(|item| item.node == node)?;
        let x = point.x - prepared.origin.x;
        let y = point.y - prepared.origin.y;
        if !x.is_finite() || !y.is_finite() { return None; }
        let hit = prepared.paragraph.get_glyph_position_at_coordinate((x,y));
        let utf16_offset = text_hit_offset(hit.position, prepared.utf16_len)?;
        let affinity = match hit.affinity {
            Affinity::Upstream => TextAffinity::Upstream,
            Affinity::Downstream => TextAffinity::Downstream,
        };
        Some(TextHit { utf16_offset, affinity })
    }
    /// Consume into a private CPU surface; errors expose no partially painted output.
    /// 消费到私有CPU Surface；错误不暴露部分绘制输出，无外部Surface写入。
    pub fn into_rgba(self) -> Result<Vec<u8>, BackendError> {
        let mut surface = surfaces::raster_n32_premul((self.width as i32, self.height as i32))
            .ok_or_else(|| invalid("surface allocation failed"))?;
        let canvas = surface.canvas();
        let mut paragraphs = self.paragraphs.into_iter();
        let mut depth = 0usize;
        for op in self.ops {
            let mut brush = Paint::default();
            brush.set_anti_alias(false);
            match op {
                PaintOp::Clear(c) => {
                    canvas.clear(native_color(c));
                }
                PaintOp::FillRect { rect, color } => {
                    brush.set_color(native_color(color));
                    canvas.draw_rect(native_rect(rect), &brush);
                }
                PaintOp::StrokeRect { rect, color, width } => {
                    if width == 0.0 {
                        continue;
                    }
                    brush.set_color(native_color(color));
                    brush.set_style(PaintStyle::Stroke);
                    brush.set_stroke_width(width);
                    canvas.draw_rect(native_rect(rect), &brush);
                }
                PaintOp::Line {
                    from,
                    to,
                    color,
                    width,
                } => {
                    if width == 0.0 {
                        continue;
                    }
                    brush.set_color(native_color(color));
                    brush.set_stroke_width(width);
                    canvas.draw_line((from.x, from.y), (to.x, to.y), &brush);
                }
                PaintOp::PushClip(r) => {
                    canvas.save();
                    canvas.clip_rect(native_rect(r), ClipOp::Intersect, false);
                    depth = depth
                        .checked_add(1)
                        .ok_or_else(|| invalid("clip depth overflow"))?;
                }
                PaintOp::PopClip => {
                    if depth == 0 {
                        return Err(invalid("clip underflow"));
                    }
                    canvas.restore();
                    depth -= 1;
                }
                PaintOp::DrawText { origin, .. } => {
                    let p = paragraphs
                        .next()
                        .ok_or_else(|| invalid("prepared text missing"))?;
                    p.paragraph.paint(canvas, (origin.x, origin.y));
                }
            }
        }
        if depth != 0 || paragraphs.next().is_some() {
            return Err(invalid("unbalanced prepared frame"));
        }
        let info = ImageInfo::new(
            (self.width as i32, self.height as i32),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        );
        let mut pixels = vec![0; self.byte_len];
        if !surface.read_pixels(&info, &mut pixels, self.row_bytes, (0, 0)) {
            return Err(invalid("surface readback failed"));
        }
        Ok(pixels)
    }
}
