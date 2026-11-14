//! E5 material (frosted glass), composed by the engine on both render paths.
//!
//! Pinned Blitz turns CSS `backdrop-filter` into the `backdrop_filter` argument of the node's
//! effect layer (`push_layer`, pushed right before the node's background), but neither the CPU
//! (`anyrender_vello_cpu`) nor the GPU (`anyrender_vello_hybrid`) backend applies it. The engine
//! therefore uses the filter only as a tag that names the node (`tag_css`), records the frame
//! into an `anyrender::recording::Scene`, and rewrites it (`compose`): at each tagged layer the
//! content recorded so far (everything painted behind the node, with the open layers closed) is
//! rasterised for the node's region, blurred and saturated here on the CPU, and drawn back as an
//! image clipped to the rounded border box, followed by the tint and the optional stroke. The
//! node's own background, borders and children then paint above it as usual.
//!
//! E5 材质（毛玻璃），两条渲染路径都由引擎合成。固定版 Blitz 把 CSS `backdrop-filter` 变成节点效果层
//! （`push_layer`，紧挨在节点背景之前压入）的 `backdrop_filter` 参数，但 CPU 与 GPU 后端都不实现它。
//! 因此引擎只把该滤镜当作标识节点的标签（`tag_css`），把整帧录入 `recording::Scene` 并改写（`compose`）：
//! 在每个带标签的层处，把此前已录内容（节点背后的全部绘制，关闭当时打开的层）按节点区域光栅化，
//! 在 CPU 上做模糊与饱和度，再作为图像按圆角边框盒裁剪画回，随后叠加着色与可选描边；节点自身的
//! 背景、边框与子节点照常绘制在其上。
use jasper_ui_engine::scene::NodeId;

/// Marker value that opens a backdrop tag (`brightness()` amount, exact in f32).
const TAG_MARKER: f32 = 3_141_592.0;
/// Bits per tag chunk; every chunk is an integer below 2^22, exact in f32.
const TAG_BITS: u32 = 22;

/// CSS `backdrop-filter` value tagging node `id`: a marker and the id in three 22-bit chunks,
/// spelled as `brightness()` amounts (never applied, see the module notes).
/// 标识节点 `id` 的 CSS `backdrop-filter` 值：标记加三段 22 位的 id，写成 `brightness()` 参数（从不生效）。
pub(crate) fn tag_css(id: NodeId) -> String {
    let mask = (1u64 << TAG_BITS) - 1;
    format!(
        "brightness({}) brightness({}) brightness({}) brightness({})",
        TAG_MARKER as u64,
        id.0 & mask,
        (id.0 >> TAG_BITS) & mask,
        id.0 >> (2 * TAG_BITS)
    )
}

/// Inverse of `tag_css` on the filter Blitz built from it; None for any other filter.
/// `tag_css` 的逆：从 Blitz 据其构造的滤镜还原节点；其他滤镜为 None。
#[cfg(feature = "cpu-pending")]
pub(crate) fn decode_tag(filter: &anyrender::Filter) -> Option<NodeId> {
    use anyrender::filters::FilterEffect;
    let nodes = filter.nodes();
    if nodes.len() != 4 {
        return None;
    }
    let mut amounts = [0f32; 4];
    for (slot, node) in amounts.iter_mut().zip(nodes) {
        let FilterEffect::ColorMatrix(matrix) = &node.effect else {
            return None;
        };
        *slot = matrix.0[0];
    }
    if amounts[0] != TAG_MARKER {
        return None;
    }
    let mut id = 0u64;
    for (i, amount) in amounts[1..].iter().enumerate() {
        if !(amount.fract() == 0.0 && *amount >= 0.0 && *amount < (1u64 << TAG_BITS) as f32) {
            return None;
        }
        id |= (*amount as u64) << (TAG_BITS * i as u32);
    }
    Some(NodeId(id))
}

/// Per-frame CPU work limit for blurred materials. Over budget, a material is painted as its
/// solid fallback instead and reported (`BackdropReport::degraded`).
/// 每帧模糊材质的 CPU 工作上限。超出预算的材质改画实底回退色并在报告中列出。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackdropBudget {
    /// Most blurred materials per frame (each one rasterises its backdrop region once more).
    /// 每帧最多模糊的材质数（每个材质多光栅化一次其背景区域）。
    pub max_nodes: u32,
    /// Most device pixels of blurred backdrop regions (node box plus 3σ margin) per frame.
    /// 每帧模糊背景区域（节点盒加 3σ 外延）的设备像素总数上限。
    pub max_pixels: u64,
}

impl BackdropBudget {
    /// 8 materials and 2^21 device pixels (about one 1920×1080 frame) per frame; see
    /// `docs/appfw-material-e5-e6.md` for the measurement behind it.
    /// 每帧 8 个材质、2^21 设备像素（约一个 1920×1080 帧）；依据见 `docs/appfw-material-e5-e6.md`。
    pub const DEFAULT: Self = Self {
        max_nodes: 8,
        max_pixels: 1 << 21,
    };
    /// Never blur: every material paints its fallback (for hosts that cannot afford it).
    /// 从不模糊：所有材质画回退色（供负担不起的宿主使用）。
    pub const FALLBACK_ONLY: Self = Self {
        max_nodes: 0,
        max_pixels: 0,
    };
}

impl Default for BackdropBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Why a material was painted as its fallback.
/// 材质画成回退色的原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackdropDegradation {
    /// `BackdropBudget::max_nodes` reached.
    NodeBudget,
    /// `BackdropBudget::max_pixels` would be exceeded.
    PixelBudget,
    /// Rasterising the backdrop region failed.
    RasterFailed,
}

/// Materials of the last painted frame, in paint order.
/// 上一个绘制帧中的材质，按绘制顺序。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackdropReport {
    /// Blurred as specified.
    /// 按规定模糊。
    pub blurred: Vec<NodeId>,
    /// Painted as the solid fallback, with the reason.
    /// 改画实底回退色，附原因。
    pub degraded: Vec<(NodeId, BackdropDegradation)>,
    /// Device pixels of blurred backdrop regions.
    /// 已模糊背景区域的设备像素数。
    pub pixels: u64,
}

// ---------------------------------------------------------------------------------------------
// Pixel kernels (premultiplied RGBA8).
// 像素内核（预乘 RGBA8）。

/// Box widths `(left, right)` of the three passes approximating a Gaussian of standard
/// deviation `sigma` px (SVG 1.1 feGaussianBlur): d = ⌊σ·3·√(2π)/4 + 0.5⌋; odd d gives three
/// centred boxes of d, even d two boxes of d offset left then right and one centred box of d+1.
/// Empty when d < 2 (no visible blur).
/// 近似标准差 `sigma` 像素高斯的三遍盒宽（SVG 1.1 feGaussianBlur）。d < 2 时为空（无可见模糊）。
pub(crate) fn box_passes(sigma: f64) -> Vec<(usize, usize)> {
    if !(sigma.is_finite() && sigma > 0.0) {
        return Vec::new();
    }
    let d = (sigma * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0 + 0.5).floor() as usize;
    if d < 2 {
        return Vec::new();
    }
    if d % 2 == 1 {
        let h = (d - 1) / 2;
        vec![(h, h); 3]
    } else {
        let h = d / 2;
        vec![(h, h - 1), (h - 1, h), (h, h)]
    }
}

/// One horizontal box pass over every row: each output is the rounded mean of the `left + 1 +
/// right` inputs around it, edges repeated (clamp). Running sums: O(width + left + right) per
/// row, independent of the box size per pixel.
/// 对每行做一遍水平盒滤波：输出为周围 `left + 1 + right` 个输入的四舍五入均值，越界取边缘值。
/// 滑动和：每行 O(width + left + right)，每像素代价与盒宽无关。
fn box_rows(src: &[u8], dst: &mut [u8], width: usize, height: usize, left: usize, right: usize) {
    let n = (left + 1 + right) as u32;
    let half = n / 2;
    let at = |row: &[u8], x: isize, c: usize| -> u32 {
        let x = x.clamp(0, width as isize - 1) as usize;
        u32::from(row[x * 4 + c])
    };
    for y in 0..height {
        let row = &src[y * width * 4..(y + 1) * width * 4];
        let out = &mut dst[y * width * 4..(y + 1) * width * 4];
        for c in 0..4 {
            let mut sum: u32 = (-(left as isize)..=right as isize)
                .map(|x| at(row, x, c))
                .sum();
            for x in 0..width {
                out[x * 4 + c] = ((sum + half) / n) as u8;
                sum += at(row, x as isize + right as isize + 1, c);
                sum -= at(row, x as isize - left as isize, c);
            }
        }
    }
}

fn transpose(src: &[u8], dst: &mut [u8], width: usize, height: usize) {
    for y in 0..height {
        for x in 0..width {
            let s = (y * width + x) * 4;
            let d = (x * height + y) * 4;
            dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
}

/// Separable Gaussian approximation (three box passes per axis, `box_passes`) of a premultiplied
/// RGBA8 image in place, edges clamped. Work is 6 passes × width × height plus a transpose each
/// way, independent of `sigma`. Premultiplied channels stay ≤ alpha (means of valid pixels).
/// 对预乘 RGBA8 图像原地做可分离高斯近似（每轴三遍盒滤波），边缘取边缘值。工作量为 6 遍 × 宽 × 高加两次转置，
/// 与 `sigma` 无关；预乘通道保持 ≤ alpha（合法像素的均值）。
pub(crate) fn gaussian_blur(pixels: &mut [u8], width: usize, height: usize, sigma: f64) {
    let passes = box_passes(sigma);
    if passes.is_empty() || width == 0 || height == 0 {
        return;
    }
    let mut cur = pixels.to_vec();
    let mut tmp = vec![0u8; pixels.len()];
    for (left, right) in &passes {
        box_rows(&cur, &mut tmp, width, height, *left, *right);
        std::mem::swap(&mut cur, &mut tmp);
    }
    transpose(&cur, &mut tmp, width, height);
    std::mem::swap(&mut cur, &mut tmp);
    for (left, right) in &passes {
        box_rows(&cur, &mut tmp, height, width, *left, *right);
        std::mem::swap(&mut cur, &mut tmp);
    }
    transpose(&cur, pixels, height, width);
}

/// CSS `saturate(s)` (Filter Effects `feColorMatrix type="saturate"`) on premultiplied RGBA8 in
/// sRGB, in place; the matrix rows sum to 1 and have no offset, so it applies to premultiplied
/// values directly. Results are clamped to 0..=alpha. `s == 1` is the identity.
/// 在 sRGB 预乘 RGBA8 上原地做 CSS `saturate(s)`；矩阵每行和为 1 且无偏移，因而可直接作用于预乘值。
/// 结果钳制到 0..=alpha；`s == 1` 为恒等。
pub(crate) fn saturate(pixels: &mut [u8], s: f32) {
    if s == 1.0 {
        return;
    }
    let m = [
        [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
    ];
    for px in pixels.chunks_exact_mut(4) {
        let (r, g, b, a) = (
            f32::from(px[0]),
            f32::from(px[1]),
            f32::from(px[2]),
            f32::from(px[3]),
        );
        for (c, row) in m.iter().enumerate() {
            px[c] = (row[0] * r + row[1] * g + row[2] * b).round().clamp(0.0, a) as u8;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Frame composition.
// 帧合成。

/// Device-pixel rectangle of a backdrop region.
#[cfg(feature = "cpu-pending")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// One material node of the frame, from its presented style and layout.
#[cfg(feature = "cpu-pending")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlassSpec {
    pub backdrop: jasper_ui_engine::style::Backdrop,
    pub radii: jasper_ui_engine::style::CornerRadii,
    /// Logical border-box size.
    pub width: f32,
    pub height: f32,
}

/// Replays recorded commands (borrowed) into `scene` under `offset`.
#[cfg(feature = "cpu-pending")]
pub(crate) fn replay(
    scene: &mut impl anyrender::PaintScene,
    commands: &[anyrender::recording::RenderCommand],
    offset: peniko::kurbo::Affine,
) {
    use anyrender::recording::RenderCommand;
    use anyrender::Paint;
    fn brush(paint: &Paint) -> anyrender::PaintRef<'_> {
        match paint {
            Paint::Solid(c) => Paint::Solid(*c),
            Paint::Gradient(g) => Paint::Gradient(g),
            Paint::Image(i) => Paint::Image(i.as_ref()),
            Paint::Resource(id) => Paint::Resource(id.clone()),
            Paint::Custom(c) => Paint::Custom(c.as_ref()),
        }
    }
    for command in commands {
        match command {
            RenderCommand::PushLayer(c) => scene.push_layer(
                c.fill,
                c.blend,
                c.alpha,
                offset * c.transform,
                &c.clip,
                c.filter.clone(),
                c.backdrop_filter.clone(),
            ),
            RenderCommand::PushClipLayer(c) => {
                scene.push_clip_layer(c.fill, offset * c.transform, &c.clip)
            }
            RenderCommand::PopLayer => scene.pop_layer(),
            RenderCommand::Stroke(c) => scene.stroke(
                &c.style,
                offset * c.transform,
                brush(&c.brush),
                c.brush_transform,
                &c.shape,
            ),
            RenderCommand::Fill(c) => scene.fill(
                c.fill,
                offset * c.transform,
                brush(&c.brush),
                c.brush_transform,
                &c.shape,
            ),
            RenderCommand::GlyphRun(c) => scene.draw_glyphs(
                &c.font_data,
                c.font_size,
                c.hint,
                &c.normalized_coords,
                c.embolden,
                &c.style,
                brush(&c.brush),
                c.brush_alpha,
                offset * c.transform,
                c.glyph_transform,
                c.glyphs.iter().copied(),
            ),
            RenderCommand::BoxShadow(c) => scene.draw_box_shadow(
                offset * c.transform,
                c.rect,
                c.brush,
                c.radius,
                c.std_dev,
            ),
        }
    }
}

/// Result of `compose`.
#[cfg(feature = "cpu-pending")]
pub(crate) struct Composed {
    pub commands: Vec<anyrender::recording::RenderCommand>,
    pub report: BackdropReport,
    /// Blob ids of the backdrop images (GPU hosts free them after the frame).
    pub images: Vec<u64>,
}

/// Rewrites a recorded frame: every effect layer tagged for a node in `specs` gets the node's
/// material drawn right after it (see the module notes). `raster` rasterises a command prefix
/// (with the given number of layers still open) into a region-sized premultiplied RGBA8 buffer.
/// Nodes are budgeted in paint order; a node over budget or whose raster fails paints its
/// fallback colour. Layers tagged for unknown nodes just lose the tag.
/// 改写录制帧：`specs` 中节点的效果层之后紧接着画该节点的材质（见模块说明）。`raster` 把命令前缀（附仍打开的层数）
/// 光栅化为区域大小的预乘 RGBA8 缓冲。按绘制顺序计入预算；超预算或光栅化失败的节点画回退色。未知节点的标签仅被去掉。
#[cfg(feature = "cpu-pending")]
pub(crate) fn compose(
    recorded: Vec<anyrender::recording::RenderCommand>,
    specs: &std::collections::BTreeMap<NodeId, GlassSpec>,
    budget: BackdropBudget,
    viewport: (u32, u32),
    mut raster: impl FnMut(&[anyrender::recording::RenderCommand], usize, Region) -> Option<Vec<u8>>,
) -> Composed {
    use anyrender::recording::{FillCommand, RenderCommand, StrokeCommand};
    use anyrender::Paint;
    use peniko::kurbo::{Affine, Rect, RoundedRect, RoundedRectRadii, Shape, Stroke};
    let mut out = Vec::with_capacity(recorded.len() + 4 * specs.len());
    let mut report = BackdropReport::default();
    let mut images = Vec::new();
    let mut depth = 0usize;
    for command in recorded {
        let RenderCommand::PushLayer(mut layer) = command else {
            match &command {
                RenderCommand::PushLayer(_) | RenderCommand::PushClipLayer(_) => depth += 1,
                RenderCommand::PopLayer => depth = depth.saturating_sub(1),
                _ => {}
            }
            out.push(command);
            continue;
        };
        let tagged = layer.backdrop_filter.take().and_then(|f| decode_tag(&f));
        let spec = tagged.and_then(|id| specs.get(&id).map(|s| (id, *s)));
        let transform = layer.transform;
        let clip = layer.clip.bounding_box();
        out.push(RenderCommand::PushLayer(layer));
        let Some((id, spec)) = spec else {
            depth += 1;
            continue;
        };
        if !(spec.width > 0.0 && spec.height > 0.0 && clip.width() > 0.0) {
            depth += 1;
            continue;
        }
        // Local → frame units of the effect layer (Blitz may pre-scale the frame).
        let factor = clip.width() / f64::from(spec.width);
        let used = crate::style_effects::used_radii(spec.width, spec.height, spec.radii);
        let r = used.map(|v| f64::from(v) * factor);
        let rounded = |rect: Rect, inset: f64| {
            RoundedRect::from_rect(
                rect.inset(-inset),
                RoundedRectRadii::new(
                    (r[0] - inset).max(0.0),
                    (r[1] - inset).max(0.0),
                    (r[2] - inset).max(0.0),
                    (r[3] - inset).max(0.0),
                ),
            )
        };
        let device_path = transform * rounded(clip, 0.0).to_path(0.1);
        let det = transform.determinant().abs();
        let bbox = device_path.bounding_box();
        let sigma = f64::from(spec.backdrop.blur) * factor * det.sqrt();
        let margin = (3.0 * sigma).ceil() + 1.0;
        let x0 = (bbox.x0 - margin).floor().max(0.0);
        let y0 = (bbox.y0 - margin).floor().max(0.0);
        let x1 = (bbox.x1 + margin).ceil().min(f64::from(viewport.0));
        let y1 = (bbox.y1 + margin).ceil().min(f64::from(viewport.1));
        let fill = |out: &mut Vec<RenderCommand>, brush: Paint, brush_transform| {
            out.push(RenderCommand::Fill(FillCommand {
                fill: peniko::Fill::NonZero,
                transform: Affine::IDENTITY,
                brush,
                brush_transform,
                shape: device_path.clone(),
            }));
        };
        let solid = |c: jasper_ui_engine::style::Color| {
            Paint::Solid(peniko::Color::from_rgba8(c.r, c.g, c.b, c.a))
        };
        if !(det > 0.0 && x1 > x0 && y1 > y0) {
            // Off screen or collapsed: nothing visible, nothing to budget.
            depth += 1;
            continue;
        }
        let region = Region {
            x: x0 as u32,
            y: y0 as u32,
            width: (x1 - x0) as u32,
            height: (y1 - y0) as u32,
        };
        let area = u64::from(region.width) * u64::from(region.height);
        let degraded = if report.blurred.len() as u32 >= budget.max_nodes {
            Some(BackdropDegradation::NodeBudget)
        } else if report.pixels + area > budget.max_pixels {
            Some(BackdropDegradation::PixelBudget)
        } else {
            None
        };
        let pixels = match degraded {
            Some(_) => None,
            // The prefix excludes the layer just pushed (it is still empty).
            None => raster(&out[..out.len() - 1], depth, region).filter(|p| {
                p.len() == region.width as usize * region.height as usize * 4
            }),
        };
        match (degraded, pixels) {
            (None, Some(mut pixels)) => {
                gaussian_blur(
                    &mut pixels,
                    region.width as usize,
                    region.height as usize,
                    sigma,
                );
                saturate(&mut pixels, spec.backdrop.saturation);
                let image = peniko::ImageData {
                    data: peniko::Blob::from(pixels),
                    format: peniko::ImageFormat::Rgba8,
                    alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
                    width: region.width,
                    height: region.height,
                };
                images.push(image.data.id());
                let brush = peniko::ImageBrush::new(image)
                    .with_extend(peniko::Extend::Pad)
                    .with_quality(peniko::ImageQuality::Low);
                fill(
                    &mut out,
                    Paint::Image(brush),
                    Some(Affine::translate((f64::from(region.x), f64::from(region.y)))),
                );
                let tint = spec.backdrop.tint;
                if tint.a > 0 {
                    fill(&mut out, solid(tint), None);
                }
                report.blurred.push(id);
                report.pixels += area;
            }
            (reason, _) => {
                fill(&mut out, solid(spec.backdrop.fallback_color()), None);
                report
                    .degraded
                    .push((id, reason.unwrap_or(BackdropDegradation::RasterFailed)));
            }
        }
        if let Some(stroke) = spec.backdrop.stroke {
            let width = f64::from(stroke.width) * factor;
            let c = stroke.color;
            out.push(RenderCommand::Stroke(StrokeCommand {
                style: Stroke::new(width),
                transform,
                brush: Paint::Solid(peniko::Color::from_rgba8(c.r, c.g, c.b, c.a)),
                brush_transform: None,
                shape: rounded(clip, width / 2.0).to_path(0.1),
            }));
        }
        depth += 1;
    }
    Composed {
        commands: out,
        report,
        images,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip_through_blitz_filters() {
        for id in [0u64, 1, 41, (1 << 22) - 1, 1 << 22, u64::MAX, 0x1234_5678_9abc_def0] {
            let css = tag_css(NodeId(id));
            assert!(css.starts_with("brightness(3141592)"), "{css}");
            #[cfg(feature = "cpu-pending")]
            {
                use anyrender::filters::FilterEffect;
                let amounts: Vec<f32> = css
                    .split(' ')
                    .map(|p| {
                        p.trim_start_matches("brightness(")
                            .trim_end_matches(')')
                            .parse::<f32>()
                            .unwrap()
                    })
                    .collect();
                let filter =
                    anyrender::Filter::linear_list(amounts.into_iter().map(FilterEffect::brightness));
                assert_eq!(decode_tag(&filter), Some(NodeId(id)));
            }
        }
        #[cfg(feature = "cpu-pending")]
        {
            use anyrender::filters::FilterEffect;
            let other = anyrender::Filter::linear_list(
                [FilterEffect::blur(4.0), FilterEffect::saturate(1.8)].into_iter(),
            );
            assert_eq!(decode_tag(&other), None);
        }
    }

    #[test]
    fn box_passes_follow_the_svg_formula() {
        assert!(box_passes(0.0).is_empty());
        assert!(box_passes(0.5).is_empty());
        // σ = 1: d = ⌊1.88 + 0.5⌋ = 2 (even).
        assert_eq!(box_passes(1.0), vec![(1, 0), (0, 1), (1, 1)]);
        // σ = 10: d = ⌊18.8 + 0.5⌋ = 19 (odd).
        assert_eq!(box_passes(10.0), vec![(9, 9); 3]);
        // The three passes' standard deviation approximates σ within 15 % (box quantisation).
        for sigma in [2.0, 5.0, 10.0, 20.0, 64.0] {
            let var: f64 = box_passes(sigma)
                .iter()
                .map(|(l, r)| {
                    let n = (l + r + 1) as f64;
                    (n * n - 1.0) / 12.0
                })
                .sum();
            assert!((var.sqrt() / sigma - 1.0).abs() < 0.15, "σ {sigma}: {}", var.sqrt());
        }
    }

    fn image(width: usize, height: usize, f: impl Fn(usize, usize) -> [u8; 4]) -> Vec<u8> {
        let mut v = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                v.extend(f(x, y));
            }
        }
        v
    }

    #[test]
    fn blur_keeps_flat_colour_and_smooths_an_edge_monotonically() {
        let (w, h) = (64, 9);
        let mut flat = image(w, h, |_, _| [40, 80, 120, 200]);
        gaussian_blur(&mut flat, w, h, 6.0);
        assert!(flat.chunks(4).all(|p| p == [40, 80, 120, 200]));
        // Black | white step at x = 32.
        let mut step = image(w, h, |x, _| if x < 32 { [0, 0, 0, 255] } else { [255; 4] });
        gaussian_blur(&mut step, w, h, 4.0);
        let row: Vec<u8> = (0..w).map(|x| step[(4 * w + x) * 4]).collect();
        assert!(row.windows(2).all(|p| p[0] <= p[1]), "{row:?}");
        assert_eq!(row[0], 0);
        assert_eq!(row[w - 1], 255);
        // Midpoint near half, and the transition spans several pixels (no hard edge).
        assert!((100..=155).contains(&row[31]) || (100..=155).contains(&row[32]), "{row:?}");
        let ramp = row.iter().filter(|v| (16..240).contains(*v)).count();
        assert!(ramp >= 8, "{row:?}");
        // Alpha stays opaque; premultiplied channels never exceed alpha.
        assert!(step.chunks(4).all(|p| p[3] == 255));
        let mut holes = image(w, h, |x, y| if (x + y) % 3 == 0 { [200, 10, 90, 220] } else { [0; 4] });
        gaussian_blur(&mut holes, w, h, 3.0);
        assert!(holes.chunks(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
    }

    #[test]
    fn blur_is_symmetric_in_both_axes() {
        let (w, h) = (41, 41);
        let mut dot = image(w, h, |x, y| if x == 20 && y == 20 { [255; 4] } else { [0; 4] });
        gaussian_blur(&mut dot, w, h, 3.0);
        let at = |x: usize, y: usize| dot[(y * w + x) * 4 + 3];
        assert!(at(20, 20) > 0);
        assert_eq!(at(20, 17), at(17, 20));
        assert!(at(20, 20) >= at(20, 22) && at(20, 22) >= at(20, 25));
    }

    #[test]
    fn saturation_matches_css_matrix() {
        let mut px = vec![200, 100, 50, 255, 255, 255, 255, 255, 100, 50, 25, 128];
        saturate(&mut px, 1.0);
        assert_eq!(px, vec![200, 100, 50, 255, 255, 255, 255, 255, 100, 50, 25, 128]);
        saturate(&mut px, 0.0);
        // Grey: luma 0.213 R + 0.715 G + 0.072 B; white stays white; premultiplied grey ≤ alpha.
        let luma = (0.213f32 * 200.0 + 0.715 * 100.0 + 0.072 * 50.0).round() as u8;
        assert_eq!(&px[0..4], &[luma, luma, luma, 255]);
        assert_eq!(&px[4..8], &[255, 255, 255, 255]);
        assert!(px[8] == px[9] && px[9] == px[10] && px[8] <= 128);
        let mut vivid = vec![200, 100, 50, 255];
        saturate(&mut vivid, 2.0);
        assert!(vivid[0] > 200 && vivid[2] < 50, "{vivid:?}");
    }
}
