//! P0-A image resources: host-injected bytes become validated, decoded sources held per view;
//! rasters for the exact device-pixel content box of each image node are cached by
//! (resource id, generation, target size, fit, alignment, colours, scale) under a host quota
//! with LRU eviction. Decoding and SVG rasterisation are delegated to audited crates
//! (`image` with the png / zune-jpeg / image-webp codecs, `resvg`/`usvg`); this module owns only
//! admission, limits, object-fit geometry, colour treatment and cache bookkeeping.
//! The engine never fetches anything: bytes (or a host-chosen file path) come from the host.
//!
//! P0-A 图片资源：宿主注入的字节经校验、解码后按视图持有；每个图片节点按其内容盒的精确设备像素尺寸光栅化，
//! 以（资源 id、代数、目标尺寸、适配、对齐、颜色、缩放）为键缓存，受宿主额度约束并按 LRU 淘汰。
//! 解码与 SVG 光栅化交给可审计的成熟库（`image` 的 png / zune-jpeg / image-webp 编解码器，`resvg`/`usvg`）；
//! 本模块只负责准入、上限、object-fit 几何、颜色处理与缓存记账。引擎从不自行获取资源：字节（或宿主选定的文件路径）由宿主提供。

use blitz_dom::node::RasterImageData;
use image::GenericImageView;
use jasper_ui_engine::scene::NodeId;
use jasper_ui_engine::style::{image_id_valid, Color, ImageStyle, ObjectFit};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Container format of an injected resource, detected from its leading bytes (never from a
/// file name). / 注入资源的容器格式，按开头字节识别（从不依据文件名）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Svg,
}

/// What the engine admitted for one resource. `natural_width/height` are logical pixels: bitmap
/// pixels divided by the declared density, or the SVG's own size.
/// 引擎对一个资源的准入结果。`natural_width/height` 为逻辑像素：位图像素除以声明的密度，或 SVG 自身尺寸。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageInfo {
    pub format: ImageFormat,
    /// Bitmap pixels; for SVG the size rounded up. / 位图像素；SVG 为向上取整的尺寸。
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub natural_width: f32,
    pub natural_height: f32,
    /// Bytes this resource holds against `ImageLimits::max_resource_bytes` (decoded RGBA for a
    /// bitmap, the source text for an SVG). / 计入 `max_resource_bytes` 的字节数（位图为解码后 RGBA，SVG 为源文本）。
    pub held_bytes: usize,
}

/// Host-configured limits of one view's image store. `None` is unlimited. The defaults bound
/// every decode (16384 px per side, 2^26 pixels, 64 MiB of encoded input) and the raster cache
/// (64 MiB, 1024 entries); the number and total size of resources are unlimited by default.
/// 单个视图图片存储的宿主额度；`None` 表示无上限。默认值限制每次解码（每边 16384 像素、2^26 像素、
/// 64 MiB 编码输入）与光栅缓存（64 MiB、1024 条）；资源个数与总字节默认无上限。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageLimits {
    /// Longest side of a decoded bitmap, an SVG and a node raster. / 解码位图、SVG 与节点光栅的最长边。
    pub max_dimension: u32,
    /// Pixels of a decoded bitmap or one node raster. / 单个解码位图或节点光栅的像素数。
    pub max_pixels: u64,
    /// Encoded input bytes of one resource (bytes or file). / 单个资源编码输入的字节数（字节或文件）。
    pub max_encoded_bytes: usize,
    /// Total `held_bytes` of all resources. / 全部资源 `held_bytes` 之和。
    pub max_resource_bytes: Option<usize>,
    /// Number of resources. / 资源个数。
    pub max_resources: Option<usize>,
    /// Bytes of cached rasters (RGBA8). / 缓存光栅的字节数（RGBA8）。
    pub max_cache_bytes: Option<usize>,
    /// Number of cached rasters. / 缓存光栅条目数。
    pub max_cache_entries: Option<usize>,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            max_dimension: 16_384,
            max_pixels: 1 << 26,
            max_encoded_bytes: 64 << 20,
            max_resource_bytes: None,
            max_resources: None,
            max_cache_bytes: Some(64 << 20),
            max_cache_entries: Some(1024),
        }
    }
}

impl ImageLimits {
    /// Positive per-decode bounds. / 每次解码的上限为正。
    pub fn valid(&self) -> bool {
        self.max_dimension > 0 && self.max_pixels > 0 && self.max_encoded_bytes > 0
    }
}

/// Structured rejection of a resource operation; the store is unchanged on every error.
/// 资源操作的结构化拒绝；任何错误下存储都不变。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// Foreign owner or closed view. / 非归属调用方或视图已关闭。
    Rejected,
    /// The id is not 1..=128 bytes of `[A-Za-z0-9._\-:/@+]`. / id 不合法。
    InvalidId,
    /// Not PNG, JPEG, WebP or (uncompressed) SVG. / 不是 PNG、JPEG、WebP 或（未压缩的）SVG。
    UnsupportedFormat,
    /// The codec rejected the data; `detail` is the library's message.
    /// 编解码器拒绝了数据；`detail` 为库给出的说明。
    Decode { format: ImageFormat, detail: String },
    /// Declared size beyond `max_dimension` / `max_pixels`, or an empty image.
    /// 声明尺寸超出 `max_dimension` / `max_pixels`，或为空图。
    TooLarge { width: u64, height: u64 },
    /// Encoded input larger than `max_encoded_bytes`. / 编码输入超过 `max_encoded_bytes`。
    EncodedTooLarge { limit: usize, required: u64 },
    /// The store would hold `required` bytes; the quota allows `limit`.
    /// 存储将持有 `required` 字节，额度只允许 `limit`。
    ResourceQuotaExceeded { limit: usize, required: usize },
    /// The store would hold `required` resources; the quota allows `limit`.
    /// 存储将持有 `required` 个资源，额度只允许 `limit` 个。
    ResourceCountExceeded { limit: usize, required: usize },
    /// Density outside 0.25..=16. / 密度不在 0.25..=16。
    InvalidDensity,
    /// Limits with a zero per-decode bound. / 每次解码上限为零。
    InvalidLimits,
    /// The host-chosen file could not be read. / 宿主选定的文件无法读取。
    Io(std::io::ErrorKind),
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected => write!(f, "image operation rejected: foreign owner or closed view"),
            Self::InvalidId => write!(f, "invalid image resource id"),
            Self::UnsupportedFormat => write!(f, "unsupported image format"),
            Self::Decode { format, detail } => write!(f, "cannot decode {format:?}: {detail}"),
            Self::TooLarge { width, height } => {
                write!(f, "image size {width}x{height} is empty or beyond the host limit")
            }
            Self::EncodedTooLarge { limit, required } => write!(
                f,
                "encoded image is {required} bytes but the host limit is {limit} bytes"
            ),
            Self::ResourceQuotaExceeded { limit, required } => write!(
                f,
                "image resources need {required} bytes but the host quota is {limit} bytes"
            ),
            Self::ResourceCountExceeded { limit, required } => write!(
                f,
                "image resources need {required} entries but the host quota is {limit}"
            ),
            Self::InvalidDensity => write!(f, "image density must be within 0.25..=16"),
            Self::InvalidLimits => write!(f, "image limits must be positive"),
            Self::Io(kind) => write!(f, "cannot read image file: {kind:?}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// Why one image node painted its placeholder (nothing: the node's own background shows) in the
/// last painted frame. Frames are never rejected for resource state; see `docs/appfw-images.md`.
/// 上一个绘制帧中某图片节点为何画占位（不画图片：显示节点自身背景）。帧从不因资源状态被拒绝，见 `docs/appfw-images.md`。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageNodeError {
    /// No resource with that id is injected. / 未注入该 id 的资源。
    Missing,
    /// The content box needs a raster beyond `max_dimension` / `max_pixels`.
    /// 内容盒所需光栅超出 `max_dimension` / `max_pixels`。
    RasterTooLarge { width: u32, height: u32 },
    /// This frame's rasters do not fit the cache quota (rasters painted this frame are never
    /// evicted to make room). / 本帧所需光栅超出缓存额度（本帧已用光栅不会为腾空间而淘汰）。
    CacheBudgetExceeded { required: usize, limit: usize },
    /// The SVG could not be rendered into the raster. / SVG 无法渲染到光栅。
    Render,
}

/// One failed image node of the last painted frame. / 上一个绘制帧中一个失败的图片节点。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageNodeReport {
    pub node: NodeId,
    pub source: String,
    pub error: ImageNodeError,
}

/// Image state after the last painted frame: failed nodes of that frame, the cache occupancy
/// and cumulative counters since the view was created.
/// 上一个绘制帧之后的图片状态：该帧失败的节点、缓存占用与视图创建以来的累计计数。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageReport {
    pub failed: Vec<ImageNodeReport>,
    pub resources: usize,
    pub resource_bytes: usize,
    pub cache_entries: usize,
    pub cache_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    /// Entries evicted by LRU to stay within the cache quota. / 为满足缓存额度按 LRU 淘汰的条目数。
    pub evictions: u64,
    pub evicted_bytes: u64,
    /// Entries dropped because their resource was replaced, removed or the quota lowered.
    /// 因资源替换、删除或额度降低而丢弃的条目数。
    pub invalidations: u64,
}

/// Decoded source of one resource. Bitmaps are kept premultiplied so resampling never bleeds
/// colour from transparent pixels. / 一个资源的解码源；位图以预乘形式保存，重采样不会从透明像素渗色。
#[derive(Clone)]
enum Source {
    Bitmap(Arc<image::RgbaImage>),
    Svg {
        text: Arc<[u8]>,
        tree: Arc<resvg::usvg::Tree>,
        uses_current_color: bool,
    },
}

#[derive(Clone)]
struct Resource {
    generation: u64,
    info: ImageInfo,
    source: Source,
}

/// Everything a node raster depends on. Floats are compared by bit pattern.
/// 节点光栅依赖的全部输入；浮点按位比较。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RasterKey {
    source: String,
    generation: u64,
    width: u32,
    height: u32,
    fit: ObjectFit,
    align: (u32, u32),
    tint: Option<[u8; 4]>,
    /// SVG `currentColor`, only for SVGs that use it. / SVG 的 `currentColor`，只对使用它的 SVG 计入。
    current_color: Option<[u8; 4]>,
    scale: u64,
}

#[derive(Clone)]
struct CacheEntry {
    raster: RasterImageData,
    bytes: usize,
    tick: u64,
}

/// One view's resources and raster cache. / 单个视图的资源与光栅缓存。
#[derive(Clone)]
pub(crate) struct ImageStore {
    limits: ImageLimits,
    resources: BTreeMap<String, Resource>,
    next_generation: u64,
    resource_bytes: usize,
    cache: HashMap<RasterKey, CacheEntry>,
    lru: BTreeMap<u64, RasterKey>,
    tick: u64,
    /// Entries used at or after this tick belong to the frame being prepared and are pinned.
    /// 本 tick 及之后使用的条目属于正在准备的帧，不可淘汰。
    frame_start: u64,
    cache_bytes: usize,
    report: ImageReport,
    /// Raster blob ids no longer referenced by the cache, for the GPU atlas to free.
    /// 已不被缓存引用的光栅 blob id，供 GPU 图集释放。
    retired: Vec<u64>,
}

/// Retired ids kept for a GPU frame; older ids beyond this are dropped (a host that never draws
/// on the GPU must not grow this list). / 为 GPU 帧保留的退役 id 上限；超出的旧 id 丢弃（从不用 GPU 的宿主不应让列表增长）。
const MAX_RETIRED: usize = 4096;

impl Default for ImageStore {
    fn default() -> Self {
        Self::new(ImageLimits::default())
    }
}

impl ImageStore {
    pub(crate) fn new(limits: ImageLimits) -> Self {
        Self {
            limits,
            resources: BTreeMap::new(),
            next_generation: 1,
            resource_bytes: 0,
            cache: HashMap::new(),
            lru: BTreeMap::new(),
            tick: 0,
            frame_start: 0,
            cache_bytes: 0,
            report: ImageReport::default(),
            retired: Vec::new(),
        }
    }

    pub(crate) fn limits(&self) -> ImageLimits {
        self.limits
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    pub(crate) fn info(&self, id: &str) -> Option<ImageInfo> {
        self.resources.get(id).map(|r| r.info)
    }

    /// Admit, decode and install (or atomically replace) one resource. On any error the store is
    /// unchanged; on success every raster of a replaced resource is invalidated.
    /// 准入、解码并安装（或原子替换）一个资源。任何错误下存储不变；成功时被替换资源的所有光栅失效。
    pub(crate) fn insert(&mut self, id: &str, bytes: &[u8], density: f32) -> Result<ImageInfo, ImageError> {
        if !image_id_valid(id) {
            return Err(ImageError::InvalidId);
        }
        if !(density.is_finite() && (0.25..=16.0).contains(&density)) {
            return Err(ImageError::InvalidDensity);
        }
        if bytes.len() > self.limits.max_encoded_bytes {
            return Err(ImageError::EncodedTooLarge {
                limit: self.limits.max_encoded_bytes,
                required: bytes.len() as u64,
            });
        }
        let (info, source) = decode(bytes, density, &self.limits)?;
        let old = self.resources.get(id).map_or(0, |r| r.info.held_bytes);
        let required = self
            .resource_bytes
            .saturating_sub(old)
            .saturating_add(info.held_bytes);
        if let Some(limit) = self.limits.max_resource_bytes {
            if required > limit {
                return Err(ImageError::ResourceQuotaExceeded { limit, required });
            }
        }
        let count = self.resources.len() + usize::from(!self.resources.contains_key(id));
        if let Some(limit) = self.limits.max_resources {
            if count > limit {
                return Err(ImageError::ResourceCountExceeded {
                    limit,
                    required: count,
                });
            }
        }
        self.purge_source(id);
        let generation = self.next_generation;
        self.next_generation += 1;
        self.resources.insert(
            id.to_owned(),
            Resource {
                generation,
                info,
                source,
            },
        );
        self.resource_bytes = required;
        Ok(info)
    }

    /// Remove a resource and invalidate its rasters; false when no such id.
    /// 删除资源并使其光栅失效；无此 id 时返回 false。
    pub(crate) fn remove(&mut self, id: &str) -> bool {
        let Some(old) = self.resources.remove(id) else {
            return false;
        };
        self.resource_bytes = self.resource_bytes.saturating_sub(old.info.held_bytes);
        self.purge_source(id);
        true
    }

    /// New limits apply to later decodes; the cache is trimmed to the new quota immediately.
    /// Existing resources are kept even when they exceed a lowered resource quota (the host
    /// removes what it no longer needs); later inserts are admitted against the new quota.
    /// 新额度作用于之后的解码；缓存立即裁剪到新额度。资源额度调低时已有资源保留（由宿主删除不再需要的），之后的插入按新额度准入。
    pub(crate) fn set_limits(&mut self, limits: ImageLimits) -> Result<(), ImageError> {
        if !limits.valid() {
            return Err(ImageError::InvalidLimits);
        }
        self.limits = limits;
        self.frame_start = self.tick.saturating_add(1);
        while self.over_cache_quota(0) {
            if !self.evict_oldest(true) {
                break;
            }
        }
        Ok(())
    }

    fn purge_source(&mut self, id: &str) {
        let doomed: Vec<RasterKey> = self
            .cache
            .keys()
            .filter(|key| key.source == id)
            .cloned()
            .collect();
        for key in doomed {
            self.drop_entry(&key);
            self.report.invalidations += 1;
        }
    }

    fn drop_entry(&mut self, key: &RasterKey) -> Option<usize> {
        let entry = self.cache.remove(key)?;
        self.lru.remove(&entry.tick);
        self.cache_bytes -= entry.bytes;
        if self.retired.len() >= MAX_RETIRED {
            self.retired.remove(0);
        }
        self.retired.push(entry.raster.data.id());
        Some(entry.bytes)
    }

    fn over_cache_quota(&self, extra_bytes: usize) -> bool {
        self.limits
            .max_cache_bytes
            .is_some_and(|limit| self.cache_bytes.saturating_add(extra_bytes) > limit)
            || self
                .limits
                .max_cache_entries
                .is_some_and(|limit| self.cache.len() + usize::from(extra_bytes > 0) > limit)
    }

    /// Evict the least recently used entry not pinned by the current frame.
    /// 淘汰当前帧未固定的最久未用条目。
    fn evict_oldest(&mut self, lru_counts: bool) -> bool {
        let Some((&tick, _)) = self.lru.iter().next() else {
            return false;
        };
        if tick >= self.frame_start {
            return false;
        }
        let key = self.lru[&tick].clone();
        if let Some(bytes) = self.drop_entry(&key) {
            if lru_counts {
                self.report.evictions += 1;
                self.report.evicted_bytes += bytes as u64;
            }
        }
        true
    }

    /// Start preparing a frame: rasters used from now on are pinned until the next frame.
    /// 开始准备一帧：此后使用的光栅在下一帧之前被固定。
    pub(crate) fn begin_frame(&mut self) {
        self.tick = self.tick.saturating_add(1);
        self.frame_start = self.tick;
        self.report.failed.clear();
    }

    pub(crate) fn fail(&mut self, node: NodeId, source: &str, error: ImageNodeError) {
        self.report.failed.push(ImageNodeReport {
            node,
            source: source.to_owned(),
            error,
        });
    }

    pub(crate) fn report(&self) -> ImageReport {
        let mut report = self.report.clone();
        report.resources = self.resources.len();
        report.resource_bytes = self.resource_bytes;
        report.cache_entries = self.cache.len();
        report.cache_bytes = self.cache_bytes;
        report
    }

    /// Blob ids of rasters dropped from the cache since the last call.
    /// 自上次调用以来从缓存丢弃的光栅 blob id。
    pub(crate) fn take_retired(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.retired)
    }

    /// The raster for one node: `width × height` device pixels covering the node's content box,
    /// with the image fitted and aligned inside at `scale`, coloured, straight-alpha RGBA8.
    /// 一个节点的光栅：覆盖内容盒的 `width × height` 设备像素，图片按 `scale` 在其中适配、对齐并处理颜色，直通 alpha RGBA8。
    pub(crate) fn raster(
        &mut self,
        style: &ImageStyle,
        current_color: Color,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Result<RasterImageData, ImageNodeError> {
        let resource = self
            .resources
            .get(&style.source)
            .ok_or(ImageNodeError::Missing)?;
        let uses_color = matches!(
            resource.source,
            Source::Svg {
                uses_current_color: true,
                ..
            }
        );
        let rgba = |c: Color| [c.r, c.g, c.b, c.a];
        let key = RasterKey {
            source: style.source.clone(),
            generation: resource.generation,
            width,
            height,
            fit: style.fit,
            align: (style.align_x.to_bits(), style.align_y.to_bits()),
            tint: style.tint.map(rgba),
            current_color: uses_color.then_some(rgba(current_color)),
            scale: scale.to_bits(),
        };
        self.tick = self.tick.saturating_add(1);
        if let Some(entry) = self.cache.get_mut(&key) {
            self.lru.remove(&entry.tick);
            entry.tick = self.tick;
            self.lru.insert(self.tick, key);
            self.report.hits += 1;
            return Ok(self.cache[&self.lru[&self.tick]].raster.clone());
        }
        self.report.misses += 1;
        let pixels = u64::from(width) * u64::from(height);
        if width == 0
            || height == 0
            || width > self.limits.max_dimension
            || height > self.limits.max_dimension
            || pixels > self.limits.max_pixels
        {
            return Err(ImageNodeError::RasterTooLarge { width, height });
        }
        let bytes = (pixels * 4) as usize;
        if let Some(limit) = self.limits.max_cache_bytes {
            if bytes > limit {
                return Err(ImageNodeError::CacheBudgetExceeded {
                    required: bytes,
                    limit,
                });
            }
        }
        while self.over_cache_quota(bytes) {
            if !self.evict_oldest(true) {
                let limit = self.limits.max_cache_bytes.unwrap_or(usize::MAX);
                return Err(ImageNodeError::CacheBudgetExceeded {
                    required: self.cache_bytes.saturating_add(bytes),
                    limit,
                });
            }
        }
        let resource = &self.resources[&style.source];
        let natural = (
            f64::from(resource.info.natural_width) * scale,
            f64::from(resource.info.natural_height) * scale,
        );
        let rect = fit_rect(
            style.fit,
            (f64::from(width), f64::from(height)),
            natural,
            (f64::from(style.align_x), f64::from(style.align_y)),
        );
        let mut premultiplied = match &resource.source {
            Source::Bitmap(bitmap) => compose_bitmap(bitmap, width, height, rect),
            Source::Svg {
                text,
                tree,
                uses_current_color,
            } => {
                let colored;
                let tree = if *uses_current_color {
                    colored = parse_svg(text, Some(current_color)).map_err(|_| ImageNodeError::Render)?;
                    &colored
                } else {
                    &**tree
                };
                compose_svg(tree, width, height, rect).ok_or(ImageNodeError::Render)?
            }
        };
        if let Some(tint) = style.tint {
            apply_tint(&mut premultiplied, tint);
        }
        demultiply(&mut premultiplied);
        let raster = RasterImageData::new(width, height, Arc::new(premultiplied));
        self.cache_bytes += bytes;
        self.cache.insert(
            key.clone(),
            CacheEntry {
                raster: raster.clone(),
                bytes,
                tick: self.tick,
            },
        );
        self.lru.insert(self.tick, key);
        Ok(raster)
    }
}

/// Detect, bound and decode. / 识别、限界并解码。
fn decode(bytes: &[u8], density: f32, limits: &ImageLimits) -> Result<(ImageInfo, Source), ImageError> {
    let too_large = |w: u64, h: u64| {
        w == 0
            || h == 0
            || w > u64::from(limits.max_dimension)
            || h > u64::from(limits.max_dimension)
            || w * h > limits.max_pixels
    };
    let bitmap_format = match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => Some(ImageFormat::Png),
        Ok(image::ImageFormat::Jpeg) => Some(ImageFormat::Jpeg),
        Ok(image::ImageFormat::WebP) => Some(ImageFormat::Webp),
        _ => None,
    };
    if let Some(format) = bitmap_format {
        let codec = match format {
            ImageFormat::Png => image::ImageFormat::Png,
            ImageFormat::Jpeg => image::ImageFormat::Jpeg,
            _ => image::ImageFormat::WebP,
        };
        let failure = |e: image::ImageError| match e {
            image::ImageError::Limits(_) => ImageError::TooLarge { width: 0, height: 0 },
            other => ImageError::Decode {
                format,
                detail: other.to_string(),
            },
        };
        let reader = || {
            let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), codec);
            let mut bounds = image::Limits::default();
            bounds.max_image_width = Some(limits.max_dimension);
            bounds.max_image_height = Some(limits.max_dimension);
            // Decoder working memory: the RGBA8 output plus the same again for conversions.
            // 解码器工作内存：RGBA8 输出再加同量的转换空间。
            bounds.max_alloc = Some(limits.max_pixels.saturating_mul(8));
            reader.limits(bounds);
            reader
        };
        // Header first: reject oversized images before any pixel allocation.
        // 先读头部：任何像素分配之前拒绝过大的图片。
        let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(bytes), codec)
            .into_dimensions()
            .map_err(failure)?;
        if too_large(u64::from(w), u64::from(h)) {
            return Err(ImageError::TooLarge {
                width: u64::from(w),
                height: u64::from(h),
            });
        }
        let decoded = reader().decode().map_err(failure)?;
        let mut rgba = decoded.into_rgba8();
        premultiply(&mut rgba);
        let held_bytes = rgba.as_raw().len();
        return Ok((
            ImageInfo {
                format,
                pixel_width: w,
                pixel_height: h,
                natural_width: w as f32 / density,
                natural_height: h as f32 / density,
                held_bytes,
            },
            Source::Bitmap(Arc::new(rgba)),
        ));
    }
    if !looks_like_svg(bytes) {
        return Err(ImageError::UnsupportedFormat);
    }
    let tree = parse_svg(bytes, None).map_err(|detail| ImageError::Decode {
        format: ImageFormat::Svg,
        detail,
    })?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w.is_finite() && h.is_finite()) || too_large(w.ceil() as u64, h.ceil() as u64) {
        return Err(ImageError::TooLarge {
            width: w.ceil() as u64,
            height: h.ceil() as u64,
        });
    }
    let uses_current_color = bytes
        .windows(12)
        .any(|w| w.eq_ignore_ascii_case(b"currentcolor"));
    Ok((
        ImageInfo {
            format: ImageFormat::Svg,
            pixel_width: w.ceil() as u32,
            pixel_height: h.ceil() as u32,
            natural_width: w,
            natural_height: h,
            held_bytes: bytes.len(),
        },
        Source::Svg {
            text: Arc::from(bytes),
            tree: Arc::new(tree),
            uses_current_color,
        },
    ))
}

/// An uncompressed SVG document: optional BOM and whitespace, then markup that contains `<svg`.
/// 未压缩的 SVG 文档：可选 BOM 与空白之后为含 `<svg` 的标记。
fn looks_like_svg(bytes: &[u8]) -> bool {
    let body = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let start = body.iter().position(|b| !b.is_ascii_whitespace());
    start.is_some_and(|i| body[i] == b'<') && body.windows(4).any(|w| w == b"<svg")
}

/// usvg without text, system fonts or nested raster images (features off); `currentColor`
/// resolves to `color` through an injected style sheet on the root element.
/// usvg 不含文字、系统字体与内嵌位图（特性关闭）；`currentColor` 经注入根元素的样式表解析为 `color`。
fn parse_svg(bytes: &[u8], color: Option<Color>) -> Result<resvg::usvg::Tree, String> {
    let mut options = resvg::usvg::Options::default();
    if let Some(c) = color {
        options.style_sheet = Some(format!(
            "svg {{ color: rgba({}, {}, {}, {}) }}",
            c.r,
            c.g,
            c.b,
            f32::from(c.a) / 255.0
        ));
    }
    resvg::usvg::Tree::from_data(bytes, &options).map_err(|e| e.to_string())
}

/// Device-pixel rect (x, y, w, h) of the fitted image inside a `box_size` box (CSS `object-fit`
/// and `object-position` as fractions). Natural size zero degrades to filling the box.
/// 适配后的图片在 `box_size` 盒内的设备像素矩形（CSS `object-fit` 与比例形式的 `object-position`）。自然尺寸为零时退化为铺满。
pub(crate) fn fit_rect(
    fit: ObjectFit,
    (bw, bh): (f64, f64),
    (nw, nh): (f64, f64),
    (ax, ay): (f64, f64),
) -> (f64, f64, f64, f64) {
    let (w, h) = if nw <= 0.0 || nh <= 0.0 {
        (bw, bh)
    } else {
        let contain = (bw / nw).min(bh / nh);
        match fit {
            ObjectFit::Fill => (bw, bh),
            ObjectFit::Contain => (nw * contain, nh * contain),
            ObjectFit::Cover => {
                let s = (bw / nw).max(bh / nh);
                (nw * s, nh * s)
            }
            ObjectFit::None => (nw, nh),
            ObjectFit::ScaleDown => {
                if nw <= bw && nh <= bh {
                    (nw, nh)
                } else {
                    (nw * contain, nh * contain)
                }
            }
        }
    };
    ((bw - w) * ax, (bh - h) * ay, w, h)
}

/// Resample the visible part of a premultiplied bitmap into a transparent `width × height`
/// raster at `rect`. Only the source region that lands inside the raster is resized, so cost
/// follows the visible area, not the natural size. 1:1 placement copies pixels exactly.
/// 把预乘位图的可见部分重采样到透明 `width × height` 光栅的 `rect` 处。只缩放落在光栅内的源区域，
/// 代价随可见面积而非自然尺寸增长；1:1 放置时逐像素精确复制。
fn compose_bitmap(source: &image::RgbaImage, width: u32, height: u32, rect: (f64, f64, f64, f64)) -> Vec<u8> {
    let mut out = vec![0u8; width as usize * height as usize * 4];
    let (sw, sh) = source.dimensions();
    let (x, y, w, h) = rect;
    if sw == 0 || sh == 0 || !(w > 0.0 && h > 0.0) {
        return out;
    }
    let (kx, ky) = (w / f64::from(sw), h / f64::from(sh));
    // Source span whose image lands inside [0, extent) on one axis.
    // 某一轴上成像落在 [0, extent) 内的源区间。
    let span = |origin: f64, k: f64, extent: u32, size: u32| -> Option<(u32, u32, i64, u32)> {
        let v0 = origin.max(0.0);
        let v1 = (origin + k * f64::from(size)).min(f64::from(extent));
        if v1 <= v0 {
            return None;
        }
        let s0 = (((v0 - origin) / k).floor().max(0.0) as u32).min(size);
        let s1 = (((v1 - origin) / k).ceil() as u32).clamp(s0, size);
        if s1 == s0 {
            return None;
        }
        let d0 = (origin + f64::from(s0) * k).round() as i64;
        let d1 = (origin + f64::from(s1) * k).round() as i64;
        let len = (d1 - d0).max(1) as u32;
        Some((s0, s1 - s0, d0, len))
    };
    let (Some((sx, cw, dx, tw)), Some((sy, ch, dy, th))) =
        (span(x, kx, width, sw), span(y, ky, height, sh))
    else {
        return out;
    };
    let crop = image::imageops::crop_imm(source, sx, sy, cw, ch);
    let scaled;
    let view: &dyn Fn(u32, u32) -> [u8; 4] = if tw == cw && th == ch {
        &|px, py| crop.get_pixel(px, py).0
    } else {
        // CatmullRom whenever an axis shrinks (its support widens with the ratio, so it averages
        // the covered area); bilinear (Triangle) when enlarging. Premultiplied input keeps
        // transparent edges clean; overshoot is clamped to alpha below.
        // 任一轴缩小时用 CatmullRom（支撑随比例加宽，相当于按覆盖面积平均）；放大时用双线性（Triangle）。
        // 预乘输入保证透明边缘干净；过冲在下方钳制到 alpha。
        let filter = if tw < cw || th < ch {
            image::imageops::FilterType::CatmullRom
        } else {
            image::imageops::FilterType::Triangle
        };
        scaled = image::imageops::resize(&*crop, tw, th, filter);
        &|px, py| scaled.get_pixel(px, py).0
    };
    for py in 0..th {
        let ty = dy + i64::from(py);
        if ty < 0 || ty >= i64::from(height) {
            continue;
        }
        for px in 0..tw {
            let tx = dx + i64::from(px);
            if tx < 0 || tx >= i64::from(width) {
                continue;
            }
            let [r, g, b, a] = view(px, py);
            let i = (ty as usize * width as usize + tx as usize) * 4;
            out[i..i + 4].copy_from_slice(&[r.min(a), g.min(a), b.min(a), a]);
        }
    }
    out
}

/// Render an SVG tree into a transparent premultiplied raster so that its own size maps onto
/// `rect`. / 把 SVG 树渲染到透明预乘光栅，使其自身尺寸映射到 `rect`。
fn compose_svg(tree: &resvg::usvg::Tree, width: u32, height: u32, rect: (f64, f64, f64, f64)) -> Option<Vec<u8>> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let size = tree.size();
    let (x, y, w, h) = rect;
    let sx = (w / f64::from(size.width())) as f32;
    let sy = (h / f64::from(size.height())) as f32;
    if !(sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0) {
        return Some(pixmap.take());
    }
    let transform = resvg::tiny_skia::Transform::from_row(sx, 0.0, 0.0, sy, x as f32, y as f32);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Some(pixmap.take())
}

fn premultiply(image: &mut image::RgbaImage) {
    for p in image.pixels_mut() {
        let a = u16::from(p.0[3]);
        for c in &mut p.0[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
}

/// Premultiplied → straight alpha (Blitz hands rasters to the renderers as straight alpha).
/// 预乘 → 直通 alpha（Blitz 以直通 alpha 把光栅交给渲染器）。
fn demultiply(pixels: &mut [u8]) {
    for p in pixels.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a == 0 {
            p[..3].fill(0);
        } else if a < 255 {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// Recolour premultiplied pixels to `tint`, keeping coverage: a' = a·tₐ, rgb' = t_rgb·a'.
/// 把预乘像素改为 `tint` 色并保留覆盖度：a' = a·tₐ，rgb' = t_rgb·a'。
fn apply_tint(pixels: &mut [u8], tint: Color) {
    let ta = u32::from(tint.a);
    for p in pixels.chunks_exact_mut(4) {
        let a = (u32::from(p[3]) * ta + 127) / 255;
        p[3] = a as u8;
        for (c, t) in p[..3].iter_mut().zip([tint.r, tint.g, tint.b]) {
            *c = ((u32::from(t) * a + 127) / 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_fn(width, height, |x, y| image::Rgba(pixel(x, y)));
        let mut out = std::io::Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn fit_rect_follows_css_object_fit() {
        let b = (100.0, 50.0);
        let n = (20.0, 20.0);
        let c = (0.5, 0.5);
        assert_eq!(fit_rect(ObjectFit::Fill, b, n, c), (0.0, 0.0, 100.0, 50.0));
        assert_eq!(fit_rect(ObjectFit::Contain, b, n, c), (25.0, 0.0, 50.0, 50.0));
        assert_eq!(fit_rect(ObjectFit::Cover, b, n, c), (0.0, -25.0, 100.0, 100.0));
        assert_eq!(fit_rect(ObjectFit::None, b, n, c), (40.0, 15.0, 20.0, 20.0));
        assert_eq!(fit_rect(ObjectFit::ScaleDown, b, n, c), (40.0, 15.0, 20.0, 20.0));
        assert_eq!(
            fit_rect(ObjectFit::ScaleDown, b, (200.0, 200.0), c),
            (25.0, 0.0, 50.0, 50.0)
        );
        assert_eq!(fit_rect(ObjectFit::Contain, b, n, (0.0, 1.0)), (0.0, 0.0, 50.0, 50.0));
        assert_eq!(fit_rect(ObjectFit::None, b, n, (1.0, 0.0)), (80.0, 0.0, 20.0, 20.0));
        // Degenerate natural size fills. / 自然尺寸退化时铺满。
        assert_eq!(fit_rect(ObjectFit::Contain, b, (0.0, 5.0), c), (0.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn premultiplied_round_trip_and_tint_keep_coverage() {
        let mut p = [200u8, 100, 50, 128];
        let mut img = image::RgbaImage::from_raw(1, 1, p.to_vec()).unwrap();
        premultiply(&mut img);
        assert_eq!(img.as_raw().as_slice(), &[100, 50, 25, 128]);
        p.copy_from_slice(img.as_raw());
        demultiply(&mut p);
        assert!(p.iter().zip([200u8, 100, 50, 128]).all(|(a, b)| a.abs_diff(b) <= 1));
        let mut t = [100u8, 50, 25, 128];
        apply_tint(&mut t, Color::new(255, 0, 0, 255));
        assert_eq!(t, [128, 0, 0, 128]);
        let mut t = [0u8, 0, 0, 255];
        apply_tint(&mut t, Color::new(0, 0, 255, 128));
        assert_eq!(t, [0, 0, 128, 128]);
    }

    #[test]
    fn decode_admits_formats_by_content_and_rejects_bad_input() {
        let mut store = ImageStore::default();
        let info = store.insert("a.png", &png(3, 2, |_, _| [1, 2, 3, 255]), 1.0).unwrap();
        assert_eq!((info.format, info.pixel_width, info.pixel_height), (ImageFormat::Png, 3, 2));
        assert_eq!(info.held_bytes, 24);
        let info = store.insert("a@2x", &png(4, 4, |_, _| [0; 4]), 2.0).unwrap();
        assert_eq!((info.natural_width, info.natural_height), (2.0, 2.0));
        assert_eq!(store.insert("bad id", b"x", 1.0), Err(ImageError::InvalidId));
        assert_eq!(store.insert("z", b"GIF89a....", 1.0), Err(ImageError::UnsupportedFormat));
        assert_eq!(store.insert("z", &[0x1f, 0x8b, 8, 0], 1.0), Err(ImageError::UnsupportedFormat));
        assert_eq!(store.insert("z", &png(1, 1, |_, _| [0; 4]), 0.0), Err(ImageError::InvalidDensity));
        let mut truncated = png(8, 8, |x, _| [x as u8, 0, 0, 255]);
        truncated.truncate(truncated.len() / 2);
        assert!(matches!(
            store.insert("z", &truncated, 1.0),
            Err(ImageError::Decode { format: ImageFormat::Png, .. })
        ));
        assert!(matches!(
            store.insert("z", b"<svg xmlns='http://www.w3.org/2000/svg' width='4'", 1.0),
            Err(ImageError::Decode { format: ImageFormat::Svg, .. })
        ));
        assert!(store.info("z").is_none());
        let mut small = ImageStore::new(ImageLimits {
            max_dimension: 4,
            ..ImageLimits::default()
        });
        assert_eq!(
            small.insert("big", &png(5, 1, |_, _| [0; 4]), 1.0),
            Err(ImageError::TooLarge { width: 5, height: 1 })
        );
        assert_eq!(
            small.insert(
                "svg",
                b"<svg xmlns='http://www.w3.org/2000/svg' width='9' height='2'/>",
                1.0
            ),
            Err(ImageError::TooLarge { width: 9, height: 2 })
        );
    }

    #[test]
    fn resource_quota_counts_replacement_once_and_failure_keeps_the_store() {
        let mut store = ImageStore::new(ImageLimits {
            max_resource_bytes: Some(64),
            max_resources: Some(2),
            ..ImageLimits::default()
        });
        store.insert("a", &png(2, 2, |_, _| [9; 4]), 1.0).unwrap();
        store.insert("a", &png(4, 4, |_, _| [9; 4]), 1.0).unwrap();
        assert_eq!(store.report().resource_bytes, 64);
        assert_eq!(
            store.insert("b", &png(1, 1, |_, _| [9; 4]), 1.0),
            Err(ImageError::ResourceQuotaExceeded { limit: 64, required: 68 })
        );
        store.insert("a", &png(1, 1, |_, _| [9; 4]), 1.0).unwrap();
        store.insert("b", &png(1, 1, |_, _| [9; 4]), 1.0).unwrap();
        assert_eq!(
            store.insert("c", &png(1, 1, |_, _| [9; 4]), 1.0),
            Err(ImageError::ResourceCountExceeded { limit: 2, required: 3 })
        );
        assert!(store.remove("a"));
        assert!(!store.remove("a"));
        assert_eq!(store.report().resource_bytes, 4);
    }

    #[test]
    fn cache_hits_evicts_lru_pins_the_frame_and_invalidates_on_replace() {
        let mut store = ImageStore::new(ImageLimits {
            max_cache_bytes: Some(2 * 16),
            ..ImageLimits::default()
        });
        store.insert("a", &png(2, 2, |_, _| [255, 0, 0, 255]), 1.0).unwrap();
        let style = ImageStyle::new("a");
        let raster = |s: &mut ImageStore, w| s.raster(&style, Color::BLACK, w, 2, 1.0);
        store.begin_frame();
        let first = raster(&mut store, 2).unwrap();
        assert_eq!(&first.data.data()[..4], &[255, 0, 0, 255]);
        let again = raster(&mut store, 2).unwrap();
        assert_eq!(first.data.id(), again.data.id());
        assert_eq!((store.report().hits, store.report().misses), (1, 1));
        // Same frame: the first entry is pinned, a third raster cannot evict it.
        // 同一帧：第一条被固定，第三个光栅不能淘汰它。
        raster(&mut store, 1).unwrap();
        assert!(matches!(
            raster(&mut store, 3),
            Err(ImageNodeError::CacheBudgetExceeded { .. })
        ));
        // Next frame: LRU evicts the oldest unpinned entry and reports it.
        // 下一帧：LRU 淘汰最旧的未固定条目并报告。
        store.begin_frame();
        raster(&mut store, 1).unwrap();
        raster(&mut store, 3).unwrap();
        assert!(store.report().evictions >= 1);
        assert!(store.report().cache_bytes <= 32);
        let retired = store.take_retired();
        assert!(retired.contains(&first.data.id()));
        // A cached raster of the current frame remains pinned while the quota shrinks to the
        // larger entry, so only the older smaller entry is discarded.
        // 当前帧已缓存的光栅在额度缩到较大条目时保持固定，只丢弃更旧的小条目。
        store.begin_frame();
        let mut big_limits = store.limits();
        big_limits.max_cache_bytes = Some(24);
        store.set_limits(big_limits).unwrap();
        store.begin_frame();
        raster(&mut store, 3).unwrap();
        assert!(store.report().evictions >= 1);
        assert!(store.report().cache_bytes <= 24);
        // Replacing the resource drops its rasters; the new pixels show.
        // 替换资源丢弃其光栅；显示新像素。
        store.insert("a", &png(2, 2, |_, _| [0, 0, 255, 255]), 1.0).unwrap();
        assert_eq!(store.report().cache_entries, 0);
        store.begin_frame();
        let blue = store.raster(&style, Color::BLACK, 2, 2, 1.0).unwrap();
        assert_eq!(&blue.data.data()[..4], &[0, 0, 255, 255]);
        assert!(store.remove("a"));
        assert_eq!(store.report().cache_entries, 0);
        assert_eq!(
            store.raster(&style, Color::BLACK, 2, 2, 1.0).unwrap_err(),
            ImageNodeError::Missing
        );
    }

    #[test]
    fn entry_count_quota_and_raster_size_limits() {
        let mut store = ImageStore::new(ImageLimits {
            max_cache_entries: Some(1),
            max_dimension: 8,
            ..ImageLimits::default()
        });
        store.insert("a", &png(1, 1, |_, _| [0, 0, 0, 255]), 1.0).unwrap();
        let style = ImageStyle::new("a");
        store.begin_frame();
        store.raster(&style, Color::BLACK, 2, 2, 1.0).unwrap();
        store.begin_frame();
        store.raster(&style, Color::BLACK, 3, 3, 1.0).unwrap();
        assert_eq!(store.report().cache_entries, 1);
        assert_eq!(store.report().evictions, 1);
        assert_eq!(
            store.raster(&style, Color::BLACK, 9, 1, 1.0).unwrap_err(),
            ImageNodeError::RasterTooLarge { width: 9, height: 1 }
        );
    }

    #[test]
    fn bitmap_compose_is_exact_at_one_to_one_and_clean_when_scaled() {
        let src = png(4, 2, |x, _| if x < 2 { [255, 0, 0, 255] } else { [0, 0, 0, 0] });
        let mut store = ImageStore::default();
        store.insert("s", &src, 1.0).unwrap();
        store.begin_frame();
        // `none` at 1:1, centred in 8x4: columns 2..4 red, 4..6 transparent.
        let style = ImageStyle::new("s").fit(ObjectFit::None);
        let r = store.raster(&style, Color::BLACK, 8, 4, 1.0).unwrap();
        let px = |x: usize, y: usize| r.data.data()[(y * 8 + x) * 4..][..4].to_vec();
        assert_eq!(px(2, 1), [255, 0, 0, 255]);
        assert_eq!(px(1, 1), [0, 0, 0, 0]);
        assert_eq!(px(4, 1), [0, 0, 0, 0]);
        assert_eq!(px(3, 0), [0, 0, 0, 0]);
        // Scaled ×4: the red/transparent edge never turns dark (premultiplied resampling).
        // 放大 4 倍：红色/透明交界不会发黑（预乘重采样）。
        let style = ImageStyle::new("s").fit(ObjectFit::Fill);
        let r = store.raster(&style, Color::BLACK, 16, 8, 1.0).unwrap();
        for p in r.data.data().chunks_exact(4) {
            if p[3] > 0 {
                assert!(p[0] >= 250 && p[1] == 0 && p[2] == 0, "{p:?}");
            }
        }
    }
}
