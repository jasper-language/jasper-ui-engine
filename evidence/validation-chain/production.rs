
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

const MAGIC: [u8; 4] = [b'J', b'U', b'E', b'B'];
const VERSION: u16 = 1;
const MAX_OPS: u32 = 1_000_000;

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

/// Returns the packed RGBA8 row size only for a nonempty row within the host budget.
/// 仅当非空 RGBA8 行满足宿主字节额度时返回精确行长。
///
/// Does not validate stride, height, backing allocation, alignment or permissions.
/// 不校验行跨度、高度、底层分配、对齐或权限；完整 Surface 接入仍需这些检查。
pub fn rgba8_row_bytes(width: u32, max_bytes: usize) -> Option<usize> {
    if width == 0 || (width as u64) > (max_bytes as u64) / 4 {
        None
    } else {
        Some(width as usize * 4)
    }
}

/// Validates positive row/height, non-overlapping stride and the last active byte.
/// 校验非空行/高度、无重叠行跨度及最后有效字节的终点。
///
/// The last row needs only its active bytes, not trailing stride padding.
/// 最后一行只要求有效字节，不额外要求最后一行尾部的填充空间。
/// This is numerical admission, not a backing-memory or permission check.
/// 这里只做数值准入，不证明底层内存存在、存活或具备访问权限。
pub fn surface_span(
    row_bytes: usize,
    height: usize,
    stride: usize,
    capacity: usize,
) -> Option<usize> {
    if row_bytes == 0 || height == 0 || stride < row_bytes {
        return None;
    }
    let prefix = match (height - 1).checked_mul(stride) {
        Some(value) => value,
        None => return None,
    };
    let end = match prefix.checked_add(row_bytes) {
        Some(value) => value,
        None => return None,
    };
    if end <= capacity {
        Some(end)
    } else {
        None
    }
}

/// Borrows one active row only after validating the entire declared surface.
/// 先校验整个声明的 Surface，再借用指定行的有效字节。
/// Padding is excluded; Rust ties the returned borrow to the host byte slice.
/// 返回值不含行间填充，Rust 将借用生命周期绑定到宿主字节切片。
pub fn surface_row(
    bytes: &[u8],
    row_bytes: usize,
    height: usize,
    stride: usize,
    row: usize,
) -> Option<&[u8]> {
    if row >= height {
        return None;
    }
    match surface_span(row_bytes, height, stride, bytes.len()) {
        None => return None,
        Some(_) => {}
    }
    let start = match row.checked_mul(stride) {
        Some(value) => value,
        None => return None,
    };
    let end = match start.checked_add(row_bytes) {
        Some(value) => value,
        None => return None,
    };
    if end > bytes.len() {
        None
    } else {
        Some(&bytes[start..end])
    }
}

/// Copies exactly one active row after complete surface and source-length validation.
/// 完整校验 Surface 与源行长度后，仅复制一行有效字节。
/// Failure leaves every destination byte unchanged; success preserves padding and other rows.
/// 失败不修改任何目标字节；成功保留 padding 与其余行。
pub fn write_surface_row(
    bytes: &mut [u8],
    row_bytes: usize,
    height: usize,
    stride: usize,
    row: usize,
    source: &[u8],
) -> bool {
    if source.len() != row_bytes {
        return false;
    }
    match surface_row(bytes, row_bytes, height, stride, row) {
        None => return false,
        Some(_) => {}
    }
    let start = row * stride;
    let end = start + row_bytes;
    bytes[start..end].copy_from_slice(source);
    true
}

/// Copies `source` into one active row starting `offset` bytes into it, after complete surface
/// validation; the segment must lie inside the active row (never in padding).
/// 完整校验 Surface 后，把 `source` 写入一行有效字节中自 `offset` 起的片段；片段须落在有效行内（不得进入 padding）。
/// Failure leaves every destination byte unchanged; success preserves all other bytes.
/// 失败不修改任何目标字节；成功保留其余全部字节。
pub fn write_surface_segment(
    bytes: &mut [u8],
    row_bytes: usize,
    height: usize,
    stride: usize,
    row: usize,
    offset: usize,
    source: &[u8],
) -> bool {
    if offset > row_bytes || source.len() > row_bytes - offset {
        return false;
    }
    match surface_row(bytes, row_bytes, height, stride, row) {
        None => return false,
        Some(_) => {}
    }
    let start = row * stride + offset;
    let end = start + source.len();
    bytes[start..end].copy_from_slice(source);
    true
}

/// Checked, immutable RGBA8 surface view over host-injected bytes.
/// 对宿主注入字节建立受检的只读 RGBA8 Surface 视图。
/// Private layout fields keep construction at the checked boundary; no allocation or handle import.
/// 布局字段私有，构造必须经过校验；不分配，也不导入外部资源句柄。
pub struct Rgba8SurfaceView<'a> {
    bytes: &'a [u8],
    width: u32,
    height: usize,
    stride: usize,
    row_bytes: usize,
}

impl<'a> Rgba8SurfaceView<'a> {
    pub fn new(bytes: &'a [u8], width: u32, height: usize, stride: usize) -> Option<Self> {
        let row_bytes = match rgba8_row_bytes(width, bytes.len()) {
            Some(value) => value,
            None => return None,
        };
        match surface_span(row_bytes, height, stride, bytes.len()) {
            None => None,
            Some(_) => Some(Self {
                bytes,
                width,
                height,
                stride,
                row_bytes,
            }),
        }
    }

    pub fn row(&self, row: usize) -> Option<&'a [u8]> {
        surface_row(self.bytes, self.row_bytes, self.height, self.stride, row)
    }

    /// Copies one active row into an equally sized caller-owned buffer.
    /// 将一行有效字节复制到等长的调用方缓冲。
    /// Invalid rows or length mismatches leave the destination unchanged.
    /// 非法行或长度不匹配时，目标缓冲完全不变。
    pub fn copy_row_to(&self, row: usize, destination: &mut [u8]) -> bool {
        let source = match self.row(row) {
            Some(value) => value,
            None => return false,
        };
        if destination.len() != source.len() {
            return false;
        }
        destination.copy_from_slice(source);
        true
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn stride(&self) -> usize {
        self.stride
    }
}

/// Packs RGB8 channels as a numeric 0x00RRGGBB software-presentation word.
/// 将 RGB8 通道编码为数值 0x00RRGGBB，供软件呈现宿主使用。
/// This does not blend alpha or prescribe the host's native byte order.
/// 不做 alpha 混色，也不规定宿主的原生字节序。
pub fn rgb8_word(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 * 65536 + green as u32 * 256 + blue as u32
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

#[derive(Clone, Debug)]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    clip_stack: Vec<Rect>,
}

impl Canvas {
    /// Reads one straight-alpha pixel; coordinates outside the framebuffer return None.
    /// 读取一个像素；坐标越界返回 None，不夹取。
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = ((y as usize * self.width as usize) + x as usize) * 4;
        Some(Color {
            r: self.pixels[index],
            g: self.pixels[index + 1],
            b: self.pixels[index + 2],
            a: self.pixels[index + 3],
        })
    }
}

impl Canvas {
    /// Takes an exactly sized packed RGBA8 buffer without allocating pixel storage.
    /// 接管尺寸精确匹配的紧密 RGBA8 缓冲，不分配像素存储。
    /// Zero dimensions and length mismatches return the original buffer unchanged.
    /// 零尺寸或长度不匹配时，原样返还输入缓冲；不归一化尺寸。
    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, Vec<u8>> {
        let row_bytes = match rgba8_row_bytes(width, pixels.len()) {
            Some(value) => value,
            None => return Err(pixels),
        };
        match surface_span(row_bytes, height as usize, row_bytes, pixels.len()) {
            Some(end) if end == pixels.len() => Ok(Self {
                width,
                height,
                pixels,
                clip_stack: Vec::new(),
            }),
            _ => Err(pixels),
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

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

/// Upper bound on host-injected font files per view.
/// 每个视图宿主注入字体文件数上限。
pub const MAX_INJECTED_FONTS: usize = 16;

/// A font stack holds at least one and at most MAX_INJECTED_FONTS files.
/// 字体栈至少一个、至多 MAX_INJECTED_FONTS 个文件。
pub fn font_stack_count_admits(count: usize) -> bool {
    count >= 1 && count <= MAX_INJECTED_FONTS
}

/// True only for the first occurrence of `id`: earlier files keep fallback priority.
/// 仅 `id` 首次出现时为真：先注入的文件保持回退优先级。
pub fn font_order_admits(seen: &[u64], id: u64) -> bool {
    let mut index: usize = 0;
    while index < seen.len() {
        if seen[index] == id {
            return false;
        }
        index += 1;
    }
    true
}

/// CSS numeric font weights span 1..=1000; anything else is rejected, never clamped.
/// CSS 数值字重范围为 1..=1000；其余值拒绝，不钳制。
pub fn font_weight_admits(weight: u16) -> bool {
    weight >= 1 && weight <= 1000
}

/// Upper bound on one declared font family name, in UTF-8 bytes.
/// 单个声明字体族名的 UTF-8 字节数上限。
pub const MAX_FONT_FAMILY_BYTES: usize = 128;

/// A family name is 1..=MAX_FONT_FAMILY_BYTES bytes with no control byte, DEL, quote or
/// backslash, so it is carried verbatim inside one CSS quoted string; rejected, never escaped.
/// 族名为 1..=MAX_FONT_FAMILY_BYTES 字节，且不含控制字节、DEL、双引号、反斜杠，
/// 因而可原样放入一个 CSS 引号串；不合规即拒绝，不转义。
pub fn font_family_admits(name: &[u8]) -> bool {
    if name.len() < 1 || name.len() > MAX_FONT_FAMILY_BYTES {
        return false;
    }
    let mut index: usize = 0;
    while index < name.len() {
        let byte = name[index];
        if byte < 0x20 || byte == 0x22 || byte == 0x5c || byte == 0x7f {
            return false;
        }
        index += 1;
    }
    true
}

/// Highest CSS weight that still gets a synthetic bold stand-in: above it CSS matching
/// already selects the face itself for bold requests.
/// 仍需合成粗体替身的最高 CSS 字重：高于它时 CSS 匹配本就为粗体请求选中该字面自身。
pub const BOLD_ALIAS_MAX_SOURCE_WEIGHT: u16 = 500;

/// One blob's faces, in order: `weights[i]` is face i's CSS weight and `group_max[i]` the
/// heaviest weight in face i's (family, style, width) group. The blob gets a stand-in only when
/// it holds at least one face, both lists agree in length, and every face is the heaviest of its
/// group with that weight at most BOLD_ALIAS_MAX_SOURCE_WEIGHT.
/// 一个 Blob 的各字面按序：`weights[i]` 为字面 i 的 CSS 字重，`group_max[i]` 为其
/// （族、样式、宽度）组中最重字重。仅当 Blob 至少含一个字面、两表等长、且每个字面都是
/// 本组最重且字重不超过 BOLD_ALIAS_MAX_SOURCE_WEIGHT 时，才为该 Blob 做替身。
pub fn bold_alias_admits(weights: &[u16], group_max: &[u16]) -> bool {
    if weights.len() < 1 || weights.len() != group_max.len() {
        return false;
    }
    let mut index: usize = 0;
    while index < weights.len() {
        if weights[index] != group_max[index] || group_max[index] > BOLD_ALIAS_MAX_SOURCE_WEIGHT {
            return false;
        }
        index += 1;
    }
    true
}

/// True only when nothing was invalidated since the cached frame was painted and the cache
/// has exactly the current extent and the admitted packed length.
/// 仅当缓存帧绘制后无任何失效，且其尺寸与当前一致、长度等于已准入的紧凑长度时为真。
pub fn frame_reuse_admits(
    dirty: bool,
    cached_width: u32,
    cached_height: u32,
    cached_len: usize,
    width: u32,
    height: u32,
    expected: usize,
) -> bool {
    !dirty && cached_width == width && cached_height == height && cached_len == expected
}

/// Damage of a produced frame relative to the previous frame this view produced.
/// 本视图产出帧相对其上一次产出帧的损伤。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameDamage {
    /// Pixel-identical to the previous frame (including a reused cached frame).
    /// 与上一帧逐像素一致（含复用缓存帧）。
    Unchanged,
    /// No comparable previous frame: first frame, extent change or host invalidation.
    /// 无可比较的上一帧：首帧、尺寸变化或宿主失效。
    Full,
    /// Smallest rectangle covering every changed pixel.
    /// 覆盖全部变化像素的最小矩形。
    Rect {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

/// Smallest inclusive span covering both the current span (if any) and `at`.
/// 覆盖当前区间（若有）与 `at` 的最小闭区间。
pub fn damage_span_extend(span: Option<(u32, u32)>, at: u32) -> (u32, u32) {
    match span {
        None => (at, at),
        Some((lo, hi)) => (if at < lo { at } else { lo }, if at > hi { at } else { hi }),
    }
}

/// Exact damage between two packed RGBA8 frames of the same extent; `previous` is None when
/// no comparable frame exists. Rows are compared whole, then scanned from both ends.
/// 同尺寸紧凑 RGBA8 两帧间的精确损伤；无可比较上一帧时 `previous` 为 None。
pub fn frame_damage(
    previous: Option<(u32, u32, &[u8])>,
    width: u32,
    height: u32,
    next: &[u8],
) -> FrameDamage {
    let Some((previous_width, previous_height, previous)) = previous else {
        return FrameDamage::Full;
    };
    let row = width as usize * 4;
    if (previous_width, previous_height) != (width, height)
        || row == 0
        || previous.len() != next.len()
        || next.len() != row * height as usize
    {
        return FrameDamage::Full;
    }
    let (mut xs, mut ys) = (None, None);
    for (y, (before, after)) in previous
        .chunks_exact(row)
        .zip(next.chunks_exact(row))
        .enumerate()
    {
        if before == after {
            continue;
        }
        let differs = |x: &usize| before[x * 4..x * 4 + 4] != after[x * 4..x * 4 + 4];
        let first = (0..width as usize).find(differs).unwrap_or(0);
        let last = (0..width as usize).rev().find(differs).unwrap_or(first);
        xs = Some(damage_span_extend(
            Some(damage_span_extend(xs, first as u32)),
            last as u32,
        ));
        ys = Some(damage_span_extend(ys, y as u32));
    }
    match (xs, ys) {
        (Some((x0, x1)), Some((y0, y1))) => FrameDamage::Rect {
            x: x0,
            y: y0,
            width: x1 - x0 + 1,
            height: y1 - y0 + 1,
        },
        _ => FrameDamage::Unchanged,
    }
}

/// True only for a nonempty rectangle lying entirely inside a `width`×`height` frame.
/// 仅当矩形非空且完全落在 `width`×`height` 帧内时为真。
pub fn damage_rect_admits(
    x: u32,
    y: u32,
    rect_width: u32,
    rect_height: u32,
    width: u32,
    height: u32,
) -> bool {
    rect_width > 0
        && rect_height > 0
        && x <= width
        && rect_width <= width - x
        && y <= height
        && rect_height <= height - y
}

/// Writes into a host surface that already holds the previous frame only the pixels `damage`
/// marks: Full writes every active row, Rect only the rectangle's segment of each covered row,
/// Unchanged nothing. All admission (surface geometry, packed length, rectangle bounds)
/// completes before the first byte is written, so false leaves the destination unchanged.
/// 向已持有上一帧的宿主 Surface 只写入 `damage` 标出的像素：Full 写全部有效行，Rect 只写覆盖各行内
/// 的矩形片段，Unchanged 不写。全部准入（Surface 几何、紧凑长度、矩形边界）先于首个字节写入完成，
/// 故返回 false 时目标不变。
pub fn write_damaged_frame(
    destination: &mut [u8],
    stride: usize,
    width: u32,
    height: u32,
    pixels: &[u8],
    damage: FrameDamage,
) -> bool {
    let Some(row_bytes) = rgba8_row_bytes(width, destination.len()) else {
        return false;
    };
    let rows = height as usize;
    if surface_span(row_bytes, rows, stride, destination.len()).is_none()
        || row_bytes.checked_mul(rows) != Some(pixels.len())
    {
        return false;
    }
    let (x, y, rect_width, rect_height) = match damage {
        FrameDamage::Unchanged => return true,
        FrameDamage::Full => (0, 0, width, height),
        FrameDamage::Rect {
            x,
            y,
            width: rect_width,
            height: rect_height,
        } => (x, y, rect_width, rect_height),
    };
    if !damage_rect_admits(x, y, rect_width, rect_height, width, height) {
        return false;
    }
    let (offset, span) = (x as usize * 4, rect_width as usize * 4);
    for row in y as usize..(y + rect_height) as usize {
        let start = row * row_bytes + offset;
        if !write_surface_segment(
            destination,
            row_bytes,
            rows,
            stride,
            row,
            offset,
            &pixels[start..start + span],
        ) {
            return false;
        }
    }
    true
}

/// Start of the display refresh period containing `now`: the latest `origin + k·interval`
/// (k ≥ 0) not after `now`. None for a zero interval or before the first tick.
/// 包含 `now` 的刷新周期起点：不晚于 `now` 的最大 `origin + k·interval`（k ≥ 0）；
/// 周期为零或首个节拍之前为 None。
pub fn frame_tick_at(origin: u64, interval: u64, now: u64) -> Option<u64> {
    if interval == 0 || now < origin {
        return None;
    }
    Some(now - (now - origin) % interval)
}

/// Pacing decision for one view against a host refresh clock (host-chosen time unit).
/// 视图相对宿主刷新时钟的节拍决定（时间单位由宿主决定）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePace {
    /// No unpainted change: do not wake for this view.
    /// 无待绘变化：不必为本视图唤醒。
    Idle,
    /// A frame is due, but this period already presented one or the first tick is ahead:
    /// wake at this tick.
    /// 有待绘变化，但本周期已呈现过或首个节拍未到：在此节拍唤醒。
    WaitUntil(u64),
    /// Render and present now for this tick, then pass it back as `presented`.
    /// 立即为此节拍绘制并呈现，之后将其作为 `presented` 传回。
    RenderNow(u64),
}

/// At most one frame per refresh period; missed periods are skipped, never caught up.
/// `presented` is the tick of the last frame the host presented for this view. None for a
/// zero interval or when the next tick would overflow the clock.
/// 每个刷新周期至多一帧；错过的周期直接跳过，不追帧。`presented` 为宿主上次为本视图呈现帧的
/// 节拍。周期为零或下一节拍超出时钟范围时为 None。
pub fn frame_pace(
    dirty: bool,
    presented: Option<u64>,
    origin: u64,
    interval: u64,
    now: u64,
) -> Option<FramePace> {
    if interval == 0 {
        return None;
    }
    if !dirty {
        return Some(FramePace::Idle);
    }
    let tick = match frame_tick_at(origin, interval, now) {
        Some(tick) => tick,
        None => return Some(FramePace::WaitUntil(origin)),
    };
    let already = match presented {
        Some(last) => last >= tick,
        None => false,
    };
    if !already {
        return Some(FramePace::RenderNow(tick));
    }
    if tick > u64::MAX - interval {
        return None;
    }
    Some(FramePace::WaitUntil(tick + interval))
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

fn is_zero_width(ch: char) -> bool {
    matches!(ch as u32,
        0x0300..=0x036F
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x200B..=0x200D
        | 0x20D0..=0x20FF
        | 0xFE00..=0xFE0F
        | 0xFE20..=0xFE2F
        | 0xE0100..=0xE01EF
    )
}

pub(crate) fn floor_utf8_boundary(text: &str, offset: usize) -> usize {
    let text_len = text.as_bytes().len();
    let limit = if offset <= text_len {
        offset
    } else {
        text_len
    };
    let mut current = limit;
    while current > 0 && !text.is_char_boundary(current) {
        current -= 1;
    }
    current
}

pub(crate) fn preedit_buffer_offset_base(
    caret: usize,
    display: usize,
    preedit_len: usize,
) -> usize {
    if display <= caret {
        display
    } else if display - caret <= preedit_len {
        caret
    } else {
        display - preedit_len
    }
}

/// A native layout offset is a source-text UTF-8 offset only when the layout holds exactly
/// the source text (same length; content equality is checked by the caller) and the offset is
/// a character boundary inside it, end included.
/// 仅当布局文本与源文本一致（长度相同；内容相等由调用方检查）且偏移为其中的字符边界（含末尾）时，
/// 原生布局偏移才是源文本 UTF-8 偏移。
pub fn text_offset_admits(
    layout_len: usize,
    source_len: usize,
    offset: usize,
    on_boundary: bool,
) -> bool {
    layout_len == source_len && offset <= source_len && on_boundary
}

/// A native layout range is a source-text UTF-8 range only when the layout holds exactly the
/// source text (same length; content equality is checked by the caller), start ≤ end ≤ the
/// source length, and both ends are character boundaries.
/// 仅当布局文本与源文本一致（长度相同；内容相等由调用方检查）、start ≤ end ≤ 源文本长度且两端均为
/// 字符边界时，原生布局区间才是源文本 UTF-8 区间。
pub fn text_range_admits(
    layout_len: usize,
    source_len: usize,
    start: usize,
    end: usize,
    start_on_boundary: bool,
    end_on_boundary: bool,
) -> bool {
    layout_len == source_len
        && start <= end
        && end <= source_len
        && start_on_boundary
        && end_on_boundary
}

/// Actions an assistive technology may request on a node; a disabled node offers none.
/// 辅助技术可对节点请求的动作；禁用节点没有任何动作。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessActions {
    /// The node accepts Engine::focus.
    /// 节点接受 Engine::focus。
    pub focus: bool,
    /// The node accepts Engine::activate.
    /// 节点接受 Engine::activate。
    pub activate: bool,
}

/// Actions of a node already in the tree (so visible): focus for an enabled focusable node,
/// activate for an enabled button; the same rules focus/activate admit.
/// 已在树中（即可见）节点的动作：启用且可聚焦者可聚焦，启用的按钮可激活；与 focus/activate 的准入规则一致。
pub fn access_actions(is_button: bool, focusable: bool, disabled: bool) -> AccessActions {
    AccessActions {
        focus: focusable && !disabled,
        activate: is_button && !disabled,
    }
}

/// How one node differs between two accessibility trees.
/// 同一节点在两份无障碍树之间的差别。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessChange {
    /// In neither tree.
    /// 两份树中都没有。
    Absent,
    /// Only in the newer tree.
    /// 只在较新的树中。
    Added,
    /// Only in the older tree.
    /// 只在较旧的树中。
    Removed,
    /// In both trees with equal content.
    /// 两份树中都有且内容相同。
    Unchanged,
    /// In both trees with different content.
    /// 两份树中都有但内容不同。
    Changed,
}

/// Classifies a node from its presence in the older and newer tree; `equal` (content equality)
/// only matters when the node is in both.
/// 按节点在旧树与新树中是否存在分类；`equal`（内容相等）只在两树都有该节点时起作用。
pub fn access_change(in_previous: bool, in_next: bool, equal: bool) -> AccessChange {
    if !in_next {
        if in_previous {
            AccessChange::Removed
        } else {
            AccessChange::Absent
        }
    } else if !in_previous {
        AccessChange::Added
    } else if equal {
        AccessChange::Unchanged
    } else {
        AccessChange::Changed
    }
}

/// What an AccessKit action request asks of a node, as far as this engine is concerned.
/// 就本引擎而言，AccessKit 动作请求要求节点做什么。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessRequest {
    /// AccessKit Focus.
    Focus,
    /// AccessKit Click (default action).
    /// AccessKit Click（默认动作）。
    Activate,
    /// Any other AccessKit action; never admitted.
    /// 其他任何 AccessKit 动作；一律不准入。
    Unsupported,
}

/// A request is admitted exactly when the node offers the matching action (`focus`/`activate`
/// as reported in the accessibility tree); unsupported requests never are.
/// 请求当且仅当节点提供对应动作（无障碍树中报告的 `focus`/`activate`）时准入；不支持的请求一律拒绝。
pub fn access_request_admits(request: AccessRequest, focus: bool, activate: bool) -> bool {
    match request {
        AccessRequest::Focus => focus,
        AccessRequest::Activate => activate,
        AccessRequest::Unsupported => false,
    }
}

/// Readback layout for a `width`×`height` RGBA8 frame: each row padded up to the 256-byte copy
/// alignment wgpu requires. Returns (padded row bytes, buffer bytes); None for an empty extent
/// or one beyond the 65535 frame limit shared with the CPU path.
/// `width`×`height` RGBA8 帧的读回布局：每行向上填充到 wgpu 要求的 256 字节拷贝对齐。返回
/// （填充后行字节数，缓冲字节数）；空尺寸或超出与 CPU 路径共用的 65535 上限时为 None。
pub fn gpu_readback_layout(width: u32, height: u32) -> Option<(u32, u64)> {
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return None;
    }
    let padded = (width * 4 + 255) / 256 * 256;
    Some((padded, padded as u64 * height as u64))
}

pub(crate) fn core_frame_bytes(width: u32, height: u32) -> Option<u64> {
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return None;
    }
    Some(width as u64 * height as u64 * 4)
}

pub(crate) fn admit_core_frame_bytes(width: u32, height: u32, max_bytes: usize) -> Option<u64> {
    let bytes = core_frame_bytes(width, height)?;
    if bytes > max_bytes as u64 {
        None
    } else {
        Some(bytes)
    }
}

pub struct UiKey {
    pub semantic_id: u64,
    pub generation: u64,
}

pub enum UiEventKind {
    Click,
    Activate,
    PointerDown,
    PointerUp,
    Hover,
    Focus,
    Key,
    TextCommit,
    ImeCommit,
    ImeCancel,
    Scroll,
    Dismiss,
    AccessibilityAction,
}

pub enum IdentityError {
    InvalidDomain,
    AlreadyLive(u64),
    GenerationExhausted(u64),
    Unknown(UiKey),
    Stale(UiKey),
    Retired(UiKey),
    InvalidSnapshot,
    UnexpectedGeneration { expected: UiKey, actual: UiKey },
}

pub(crate) fn next_ui_identity(
    semantic_id: u64,
    last_generation: u64,
    live: bool,
) -> Result<UiKey, IdentityError> {
    if live {
        return Err(IdentityError::AlreadyLive(semantic_id));
    }
    let Some(generation) = last_generation.checked_add(1) else {
        return Err(IdentityError::GenerationExhausted(semantic_id));
    };
    Ok(UiKey {
        semantic_id,
        generation,
    })
}

pub enum UiPermission {
    Pointer,
    Activate,
    Focus,
    Keyboard,
    TextEdit,
    Ime,
    Scroll,
    Dismiss,
    AccessibilityAction,
}

impl UiEventKind {
    pub const fn required_permission(self) -> UiPermission {
        match self {
            Self::Click | Self::Activate => UiPermission::Activate,
            Self::PointerDown | Self::PointerUp | Self::Hover => UiPermission::Pointer,
            Self::Focus => UiPermission::Focus,
            Self::Key => UiPermission::Keyboard,
            Self::TextCommit => UiPermission::TextEdit,
            Self::ImeCommit | Self::ImeCancel => UiPermission::Ime,
            Self::Scroll => UiPermission::Scroll,
            Self::Dismiss => UiPermission::Dismiss,
            Self::AccessibilityAction => UiPermission::AccessibilityAction,
        }
    }
}

pub(crate) enum AuthorityFailure {
    IdentityDomain,
    Owner,
    Session,
    Revision,
}

pub(crate) fn authority_failure_from_matches(
    identity_domain_matches: bool,
    owner_matches: bool,
    session_matches: bool,
    revision_matches: bool,
) -> Option<AuthorityFailure> {
    if !identity_domain_matches {
        return Some(AuthorityFailure::IdentityDomain);
    }
    if !owner_matches {
        return Some(AuthorityFailure::Owner);
    }
    if !session_matches {
        return Some(AuthorityFailure::Session);
    }
    if !revision_matches {
        return Some(AuthorityFailure::Revision);
    }
    None
}

pub(crate) enum PermissionFailure {
    UnknownContext,
    Denied,
}

pub(crate) fn permission_failure(
    context_named: bool,
    permission_granted: bool,
) -> Option<PermissionFailure> {
    if !context_named {
        return Some(PermissionFailure::UnknownContext);
    }
    if !permission_granted {
        return Some(PermissionFailure::Denied);
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Image,
    Svg,
    Font,
    Icon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceAdmission {
    Prepare,
    Commit,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiResourceTransition {
    Prepare,
    NoOp,
    Commit,
    ReleaseClear,
    ReleaseKeepPrepared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiResourceTransitionError {
    KindChanged,
    StaleGeneration,
    GenerationNotPrepared,
    ReleasedGeneration,
    CommitMismatch,
    ReleaseMismatch,
}

fn resource_transition(
    highest_present: bool,
    highest_kind: ResourceKind,
    highest_generation: u64,
    active_present: bool,
    active_kind: ResourceKind,
    active_generation: u64,
    prepared_present: bool,
    prepared_kind: ResourceKind,
    prepared_generation: u64,
    next_kind: ResourceKind,
    next_generation: u64,
    admission: ResourceAdmission,
) -> Result<UiResourceTransition, UiResourceTransitionError> {
    if !highest_present {
        return match admission {
            ResourceAdmission::Prepare => Ok(UiResourceTransition::Prepare),
            ResourceAdmission::Commit => Err(UiResourceTransitionError::CommitMismatch),
            ResourceAdmission::Release => Err(UiResourceTransitionError::ReleaseMismatch),
        };
    }
    let same_kind = matches!(
        (next_kind, highest_kind),
        (ResourceKind::Image, ResourceKind::Image)
            | (ResourceKind::Svg, ResourceKind::Svg)
            | (ResourceKind::Font, ResourceKind::Font)
            | (ResourceKind::Icon, ResourceKind::Icon)
    );
    if same_kind {
        if next_generation < highest_generation
            && !matches!(admission, ResourceAdmission::Release)
        {
            return Err(UiResourceTransitionError::StaleGeneration);
        }
        if next_generation > highest_generation
            && !matches!(admission, ResourceAdmission::Prepare)
        {
            return Err(UiResourceTransitionError::GenerationNotPrepared);
        }
        match admission {
            ResourceAdmission::Prepare => {
                if next_generation == highest_generation {
                    let active_matches = active_present
                        && matches!(
                            (active_kind, next_kind),
                            (ResourceKind::Image, ResourceKind::Image)
                                | (ResourceKind::Svg, ResourceKind::Svg)
                                | (ResourceKind::Font, ResourceKind::Font)
                                | (ResourceKind::Icon, ResourceKind::Icon)
                        )
                        && active_generation == next_generation;
                    let prepared_matches = prepared_present
                        && matches!(
                            (prepared_kind, next_kind),
                            (ResourceKind::Image, ResourceKind::Image)
                                | (ResourceKind::Svg, ResourceKind::Svg)
                                | (ResourceKind::Font, ResourceKind::Font)
                                | (ResourceKind::Icon, ResourceKind::Icon)
                        )
                        && prepared_generation == next_generation;
                    if prepared_matches || active_matches {
                        return Ok(UiResourceTransition::NoOp);
                    }
                    if !active_present && !prepared_present {
                        return Err(UiResourceTransitionError::ReleasedGeneration);
                    }
                }
                Ok(UiResourceTransition::Prepare)
            }
            ResourceAdmission::Commit => {
                if prepared_present
                    && matches!(
                        (prepared_kind, next_kind),
                        (ResourceKind::Image, ResourceKind::Image)
                            | (ResourceKind::Svg, ResourceKind::Svg)
                            | (ResourceKind::Font, ResourceKind::Font)
                            | (ResourceKind::Icon, ResourceKind::Icon)
                    )
                    && prepared_generation == next_generation
                {
                    Ok(UiResourceTransition::Commit)
                } else {
                    Err(UiResourceTransitionError::CommitMismatch)
                }
            }
            ResourceAdmission::Release => {
                if !active_present
                    || !matches!(
                        (active_kind, next_kind),
                        (ResourceKind::Image, ResourceKind::Image)
                            | (ResourceKind::Svg, ResourceKind::Svg)
                            | (ResourceKind::Font, ResourceKind::Font)
                            | (ResourceKind::Icon, ResourceKind::Icon)
                    )
                    || active_generation != next_generation
                {
                    return Err(UiResourceTransitionError::ReleaseMismatch);
                }
                if prepared_present {
                    Ok(UiResourceTransition::ReleaseKeepPrepared)
                } else {
                    Ok(UiResourceTransition::ReleaseClear)
                }
            }
        }
    } else {
        Err(UiResourceTransitionError::KindChanged)
    }
}

/// IEEE 754 binary64 bit patterns used by the skew decision.
/// 斜切判定使用的 IEEE 754 binary64 位模式。
pub const F64_ONE_BITS: u64 = 0x3ff0_0000_0000_0000;
pub const F64_SIGN_BIT: u64 = 0x8000_0000_0000_0000;
pub const F64_MAGNITUDE_BITS: u64 = 0x7fff_ffff_ffff_ffff;

/// Takes the `f64::to_bits` of a glyph transform `[a, b, c, d, e, f]` and returns the bits of
/// the horizontal skew `c` to draw with: sign-mirrored when the transform is exactly a pure
/// horizontal skew (a = d = 1, b = e = f = ±0), otherwise unchanged. Other coefficients are
/// never altered.
/// 输入字形变换 `[a, b, c, d, e, f]` 各系数的 `f64::to_bits`，返回应使用的水平斜切 `c` 的位：
/// 变换恰为纯水平斜切（a = d = 1，b = e = f = ±0）时符号取反，否则不变。其余系数从不改动。
pub fn upright_skew_x(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    if a == F64_ONE_BITS
        && (b & F64_MAGNITUDE_BITS) == 0
        && d == F64_ONE_BITS
        && (e & F64_MAGNITUDE_BITS) == 0
        && (f & F64_MAGNITUDE_BITS) == 0
    {
        c ^ F64_SIGN_BIT
    } else {
        c
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneUpdateError {
    OwnerMismatch,
    Closed,
    EmptyFrame,
    StaleVersion {
        expected: u64,
        current: u64,
    },
    InvalidSuccessor {
        expected: u64,
        next: u64,
    },
    SourceRejected,
    NativeRejected,
    /// The updated scene would hold `required` nodes; the host quota allows `limit`.
    /// 更新后的场景需要 `required` 个节点，宿主额度只允许 `limit` 个。
    NodeQuotaExceeded {
        limit: usize,
        required: usize,
    },
    /// The updated scene would hold `required` bytes of text and semantic strings; the host
    /// quota allows `limit`.
    /// 更新后的场景需要 `required` 字节文字与语义字符串，宿主额度只允许 `limit` 字节。
    TextQuotaExceeded {
        limit: usize,
        required: usize,
    },
}

/// Require a successful source frame at the exact successor and a detached native
/// candidate before the caller may replace the active view.
pub(crate) fn complete_scene_frame(
    source_ok: bool,
    source_version: u64,
    next: u64,
    candidate_ready: bool,
) -> Result<(), SceneUpdateError> {
    if !source_ok || source_version != next {
        return Err(SceneUpdateError::SourceRejected);
    }
    if !candidate_ready {
        return Err(SceneUpdateError::NativeRejected);
    }
    Ok(())
}

/// Run the detached frame stages in their only legal order: the builder receives only a source
/// that succeeded at the exact successor, and neither stage is handed the active view's storage.
/// The caller replaces the active view after `Ok`, so a stage panic leaves nothing to undo.
pub(crate) fn order_scene_frame<S, C>(
    next: u64,
    apply_source: impl FnOnce() -> (bool, u64, S),
    build_candidate: impl FnOnce(S) -> Option<C>,
) -> Result<C, SceneUpdateError> {
    let (source_ok, source_version, source) = apply_source();
    let candidate = if source_ok && source_version == next {
        build_candidate(source)
    } else {
        None
    };
    if let Err(error) = complete_scene_frame(source_ok, source_version, next, candidate.is_some()) {
        return Err(error);
    }
    match candidate {
        Some(candidate) => Ok(candidate),
        None => Err(SceneUpdateError::NativeRejected),
    }
}

/// Interaction state one frame carries from the active view into its detached candidate.
pub(crate) struct CarriedInteraction<I, V, S, P, C> {
    viewport: V,
    scroll: S,
    pressed: Option<I>,
    focused: Option<I>,
    pointer: Option<P>,
    frame_cache: C,
}

/// The candidate's own writes. `None` means it lost its document or a node mapping.
pub(crate) trait RestoreCandidate<I, V, S, P, C> {
    fn restore_viewport(&mut self, viewport: V, scroll: S) -> Option<()>;
    fn press_admitted(&self, id: I) -> bool;
    fn restore_press(&mut self, id: I) -> Option<()>;
    fn focus_admitted(&self, id: I) -> bool;
    fn restore_focus(&mut self, id: I) -> Option<()>;
    fn restore_hover(&mut self, pointer: P) -> Option<()>;
    fn resolve_restored(&mut self) -> Option<()>;
    fn adopt_pointer_and_cache(&mut self, pointer: Option<P>, frame_cache: C);
}

/// Replay carried state in its only legal order: viewport, then each identity the candidate still
/// admits, then hover, then one resolve; the pointer and frame cache are adopted only after that.
/// A failed write rejects the whole candidate, whereas an identity it no longer admits just drops.
/// 按唯一合法顺序重放：视口、候选仍准入的身份、悬停、一次解析，之后才采纳指针与帧缓存。
/// 写失败则整个候选作废；候选不再准入的身份只是丢弃。
fn restore_candidate_interaction<I: Copy, V, S, P: Copy, C, T: RestoreCandidate<I, V, S, P, C>>(
    candidate: &mut T,
    carried: CarriedInteraction<I, V, S, P, C>,
) -> Option<()> {
    let CarriedInteraction {
        viewport,
        scroll,
        pressed,
        focused,
        pointer,
        frame_cache,
    } = carried;
    candidate.restore_viewport(viewport, scroll)?;
    if let Some(id) = pressed {
        if candidate.press_admitted(id) {
            candidate.restore_press(id)?;
        }
    }
    if let Some(id) = focused {
        if candidate.focus_admitted(id) {
            candidate.restore_focus(id)?;
        }
    }
    if let Some(position) = pointer {
        candidate.restore_hover(position)?;
    }
    candidate.resolve_restored()?;
    candidate.adopt_pointer_and_cache(pointer, frame_cache);
    Some(())
}

/// Wire-visible identity, not a secret capability or a backing-memory owner.
/// 可在线上传输的身份，不是秘密能力凭证，也不拥有底层内存。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceKey {
    pub session: u64,
    pub slot: u64,
    pub generation: u64,
}

/// One host-owned slot. Never duplicate a live gate for the same session/slot pair.
/// 一个宿主持有的槽位；同一会话/槽位不得复制或重建并行存活的 gate。
/// The host supplies a trusted caller session; client-supplied identity is insufficient.
/// caller_session 必须来自宿主认证上下文，不能直接信任客户端自报值。
pub struct ResourceGate {
    session: u64,
    slot: u64,
    generation: u64,
    live: bool,
}

impl ResourceGate {
    pub fn new(session: u64, slot: u64) -> Self {
        Self {
            session,
            slot,
            generation: 0,
            live: false,
        }
    }

    /// Issues the next generation only to the owning session and only while vacant.
    /// 仅在槽位空闲时向所属会话签发下一代；代数耗尽永久拒绝，不回绕。
    pub fn issue(&mut self, caller_session: u64) -> Option<ResourceKey> {
        if caller_session != self.session || self.live || self.generation == u64::MAX {
            return None;
        }
        self.generation += 1;
        self.live = true;
        Some(ResourceKey {
            session: self.session,
            slot: self.slot,
            generation: self.generation,
        })
    }

    pub fn allows(&self, caller_session: u64, key: ResourceKey) -> bool {
        self.live
            && self.generation != 0
            && caller_session == self.session
            && key.session == self.session
            && key.slot == self.slot
            && key.generation == self.generation
    }

    /// Revocation is idempotent rejection after the first success; it does not free storage.
    /// 首次撤销成功后重复调用拒绝；此方法不释放实际存储，也不取消在途操作。
    pub fn revoke(&mut self, caller_session: u64, key: ResourceKey) -> bool {
        if !self.allows(caller_session, key) {
            return false;
        }
        self.live = false;
        true
    }
    /// Host shutdown permanently retires this gate; clients must not control this call.
    /// 宿主关闭时永久退役此 gate；不得把该管理调用交给未认证客户端。
    pub fn retire(&mut self) {
        self.generation = u64::MAX;
        self.live = false;
    }
}

/// Owned UI byte storage behind a persistent session/slot gate.
/// 在持久会话/槽位 gate 后持有 UI 字节存储；可供后续字体或其他字节资源服务使用。
/// Installation moves a Vec without copying; release returns ownership to the host.
/// 安装仅转移 Vec，不复制；释放将所有权返还宿主，不操作内核句柄。
pub struct UiBytesSlot {
    gate: ResourceGate,
    payload: Option<Vec<u8>>,
}

impl UiBytesSlot {
    pub fn new(session: u64, slot: u64) -> Self {
        Self {
            gate: ResourceGate::new(session, slot),
            payload: None,
        }
    }

    pub fn install(&mut self, caller_session: u64, bytes: Vec<u8>) -> Result<ResourceKey, Vec<u8>> {
        match self.gate.issue(caller_session) {
            None => Err(bytes),
            Some(key) => {
                self.payload = Some(bytes);
                Ok(key)
            }
        }
    }

    pub fn bytes(&self, caller_session: u64, key: ResourceKey) -> Option<&[u8]> {
        if !self.gate.allows(caller_session, key) {
            return None;
        }
        match self.payload.as_ref() {
            Some(bytes) => Some(bytes.as_slice()),
            None => None,
        }
    }

    /// Rust prevents releasing through &mut while a returned slice remains borrowed.
    /// 返回切片仍被借用时，Rust 禁止经 &mut 释放；不等同外部 DMA/IPC 在途取消。
    pub fn release(&mut self, caller_session: u64, key: ResourceKey) -> Option<Vec<u8>> {
        if self.gate.revoke(caller_session, key) {
            self.payload.take()
        } else {
            None
        }
    }
    /// Host shutdown returns any owned payload and permanently disables installation.
    /// 宿主关闭时取回已有载荷，并永久禁止再次安装；不取消外部在途请求。
    pub fn retire(&mut self) -> Option<Vec<u8>> {
        self.gate.retire();
        self.payload.take()
    }
}

/// A fill is admitted only when nonempty and wholly inside the node; it is rejected, never clipped.
/// 填充只有非空且完全落在节点内才准入；越界拒绝，不裁剪。
pub fn paint_rect_admits(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    node_width: u32,
    node_height: u32,
) -> bool {
    width > 0
        && height > 0
        && width <= node_width
        && x <= node_width - width
        && height <= node_height
        && y <= node_height - height
}

/// The command count from an entry's return value, when it fits the buffer.
/// 入口返回值所表示的命令数（须不超过缓冲区容量）。
pub fn paint_count_admits(returned: i32, capacity: u32) -> Option<u32> {
    if returned < 0 {
        return None;
    }
    let count = returned as u32;
    if count > capacity {
        return None;
    }
    Some(count)
}

/// The pointer a base-plus-addend relocation writes; site and target must both lie in the image.
/// 「基址 + 加数」重定位写入的指针；写入位置与目标都必须落在映像内。
pub fn relocation_value(base: u64, offset: u32, addend: u64, image_size: u32) -> Option<u64> {
    if image_size < 8 || offset > image_size - 8 {
        return None;
    }
    if addend > image_size as u64 || addend > u64::MAX - base {
        return None;
    }
    Some(base + addend)
}

/// Logical presentation-surface state. A suspended surface may be resumed; a retired one
/// never can.
/// 逻辑呈现 Surface 状态。暂停状态可恢复；退休状态永不恢复。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceState {
    Active,
    Suspended,
    Retired,
}

/// Current extent and generation visible to the owning host.
/// 所属宿主可见的当前尺寸与代数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceStatus {
    pub state: SurfaceState,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
}

/// Why a lifecycle transition was rejected. Rejections leave the transition state unchanged.
/// 生命周期迁移被拒原因；拒绝时迁移状态保持不变。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceLifecycleError {
    WrongOwner,
    WrongState,
    StaleGeneration,
    GenerationExhausted,
}

/// One host-owned surface generation. It contains no native handle and performs no I/O.
/// 一个宿主持有的 Surface 代数；不含原生句柄，也不执行 I/O。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceLifecycle {
    owner: u64,
    state: SurfaceState,
    generation: u64,
    width: u32,
    height: u32,
}

impl SurfaceLifecycle {
    /// True only for the exact live generation of the owning host.
    /// 仅当宿主身份与当前活动代数精确匹配时为真。
    pub fn render_allowed(&self, caller: u64, generation: u64) -> bool {
        caller == self.owner
            && matches!(self.state, SurfaceState::Active)
            && self.generation == generation
    }

    /// True while the owning host may submit work without carrying an epoch. Public rendering
    /// entry points use this compatibility view; generation-checked entry points are preferred.
    /// 所属宿主无需携带代数即可提交工作时的兼容视图；优先使用携带代数的渲染入口。
    pub(crate) fn active_for_owner(&self, caller: u64) -> bool {
        caller == self.owner && matches!(self.state, SurfaceState::Active)
    }
}

impl SurfaceLifecycle {
    /// Initial active host surface generation.
    /// 初始活动宿主 Surface 代数。
    pub const fn new(owner: u64, width: u32, height: u32) -> Self {
        Self {
            owner,
            state: SurfaceState::Active,
            generation: 1,
            width,
            height,
        }
    }

    pub fn status(&self, caller: u64) -> Option<SurfaceStatus> {
        if caller != self.owner {
            return None;
        }
        Some(SurfaceStatus {
            state: self.state,
            generation: self.generation,
            width: self.width,
            height: self.height,
        })
    }

    /// Replace the host extent. The old generation immediately becomes stale.
    /// 替换宿主尺寸；旧代数立即失效。
    pub fn reconfigure(
        &mut self,
        caller: u64,
        expected_generation: u64,
        width: u32,
        height: u32,
    ) -> Result<u64, SurfaceLifecycleError> {
        self.require_active(caller, expected_generation)?;
        let next = self.next_generation()?;
        self.generation = next;
        self.width = width;
        self.height = height;
        Ok(next)
    }

    /// Release host presentation resources. A stale callback cannot suspend a successor.
    /// 释放宿主呈现资源；迟到回调不能暂停后继 Surface。
    pub fn suspend(
        &mut self,
        caller: u64,
        expected_generation: u64,
    ) -> Result<u64, SurfaceLifecycleError> {
        self.require_active(caller, expected_generation)?;
        let next = self.next_generation()?;
        self.generation = next;
        self.state = SurfaceState::Suspended;
        Ok(next)
    }

    /// Recreate host presentation resources for a new generation.
    /// 为新代数重建宿主呈现资源。
    pub fn resume(
        &mut self,
        caller: u64,
        expected_generation: u64,
        width: u32,
        height: u32,
    ) -> Result<u64, SurfaceLifecycleError> {
        self.require(caller, expected_generation)?;
        if !matches!(self.state, SurfaceState::Suspended) {
            return Err(SurfaceLifecycleError::WrongState);
        }
        let next = self.next_generation()?;
        self.generation = next;
        self.width = width;
        self.height = height;
        self.state = SurfaceState::Active;
        Ok(next)
    }

    /// Permanently retire this host surface. Repeated retirement is rejected.
    /// 永久退休该宿主 Surface；重复退休被拒绝。
    pub fn retire(
        &mut self,
        caller: u64,
        expected_generation: u64,
    ) -> Result<(), SurfaceLifecycleError> {
        self.require(caller, expected_generation)?;
        if matches!(self.state, SurfaceState::Retired) {
            return Err(SurfaceLifecycleError::WrongState);
        }
        if self.generation < u64::MAX {
            self.generation += 1;
        }
        self.state = SurfaceState::Retired;
        Ok(())
    }

    /// Public owner/state retirement compatibility path. It has no generation guard.
    /// 公开 owner/state 兼容退休路径；不携带代数保护。
    pub(crate) fn retire_current(&mut self, caller: u64) -> bool {
        if caller != self.owner || matches!(self.state, SurfaceState::Retired) {
            return false;
        }
        if self.generation < u64::MAX {
            self.generation += 1;
        }
        self.state = SurfaceState::Retired;
        true
    }

    fn require_active(
        &self,
        caller: u64,
        expected_generation: u64,
    ) -> Result<(), SurfaceLifecycleError> {
        self.require(caller, expected_generation)?;
        if !matches!(self.state, SurfaceState::Active) {
            return Err(SurfaceLifecycleError::WrongState);
        }
        Ok(())
    }

    fn require(&self, caller: u64, expected_generation: u64) -> Result<(), SurfaceLifecycleError> {
        if caller != self.owner {
            return Err(SurfaceLifecycleError::WrongOwner);
        }
        if expected_generation != self.generation {
            return Err(SurfaceLifecycleError::StaleGeneration);
        }
        Ok(())
    }

    fn next_generation(&self) -> Result<u64, SurfaceLifecycleError> {
        self.generation
            .checked_add(1)
            .ok_or(SurfaceLifecycleError::GenerationExhausted)
    }
}

/// Why a coordinated step did not advance. `Lifecycle` means the engine was never asked.
/// 协调步骤未推进的原因；`Lifecycle` 表示引擎根本未被调用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceStepError {
    Lifecycle(SurfaceLifecycleError),
    EngineRejected,
}

/// A lifecycle, the engine work that goes with each of its steps, and whether a texture frame
/// rendered at the current generation still waits for the host's present.
/// 生命周期、每一步配套的引擎工作，以及当前代数渲染的纹理帧是否仍在等待宿主呈现。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceCoordinator {
    surface: SurfaceLifecycle,
    present_pending: bool,
}

impl SurfaceCoordinator {
    pub fn new(owner: u64, width: u32, height: u32) -> Self {
        Self {
            surface: SurfaceLifecycle::new(owner, width, height),
            present_pending: false,
        }
    }

    pub fn status(&self, caller: u64) -> Option<SurfaceStatus> {
        self.surface.status(caller)
    }

    pub fn render_allowed(&self, caller: u64, generation: u64) -> bool {
        self.surface.render_allowed(caller, generation)
    }

    pub(crate) fn active_for_owner(&self, caller: u64) -> bool {
        self.surface.active_for_owner(caller)
    }

    pub fn present_pending(&self) -> bool {
        self.present_pending
    }

    /// Run `engine` only if the lifecycle admits the step, then commit the step. The step is taken
    /// on a copy first, so an admitted step cannot fail after the engine has done its work.
    /// 仅当生命周期准入时才运行 `engine`，随后提交。先在副本上迁移，因此已准入的步骤不会在引擎完成后失败。
    pub fn reconfigure(
        &mut self,
        caller: u64,
        expected_generation: u64,
        width: u32,
        height: u32,
        engine: impl FnOnce() -> bool,
    ) -> Result<u64, SurfaceStepError> {
        let mut next = self.surface;
        let reconfigured = match next.reconfigure(caller, expected_generation, width, height) {
            Ok(generation) => generation,
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        };
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(reconfigured)
    }

    pub fn suspend(
        &mut self,
        caller: u64,
        expected_generation: u64,
        engine: impl FnOnce() -> bool,
    ) -> Result<u64, SurfaceStepError> {
        let mut next = self.surface;
        let suspended = match next.suspend(caller, expected_generation) {
            Ok(generation) => generation,
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        };
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(suspended)
    }

    pub fn resume(
        &mut self,
        caller: u64,
        expected_generation: u64,
        width: u32,
        height: u32,
        engine: impl FnOnce() -> bool,
    ) -> Result<u64, SurfaceStepError> {
        let mut next = self.surface;
        let resumed = match next.resume(caller, expected_generation, width, height) {
            Ok(generation) => generation,
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        };
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(resumed)
    }

    pub fn retire(
        &mut self,
        caller: u64,
        expected_generation: u64,
        engine: impl FnOnce() -> bool,
    ) -> Result<(), SurfaceStepError> {
        let mut next = self.surface;
        match next.retire(caller, expected_generation) {
            Ok(()) => {}
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        }
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(())
    }

    /// Retirement without a generation, for cleanup paths that hold no current surface.
    /// 不带代数的退休，用于不持有当前 Surface 的清理路径。
    pub fn close(&mut self, caller: u64, engine: impl FnOnce() -> bool) -> bool {
        let mut next = self.surface;
        if !next.retire_current(caller) {
            return false;
        }
        if !engine() {
            return false;
        }
        self.adopt(next);
        true
    }

    /// The pixels waiting for a present were superseded; the host must not present them.
    /// 等待呈现的像素已被取代，宿主不得再呈现。
    pub fn frame_superseded(&mut self) {
        self.present_pending = false;
    }

    /// Record a texture frame rendered at exactly the live generation.
    /// 记录恰在当前活动代数渲染的纹理帧。
    pub fn texture_rendered(&mut self, caller: u64, generation: u64) -> bool {
        if !self.surface.render_allowed(caller, generation) {
            return false;
        }
        self.present_pending = true;
        true
    }

    /// Acknowledge the host's present of the pending frame at exactly the live generation.
    /// 确认宿主恰在当前活动代数呈现了待呈现帧。
    pub fn presented(&mut self, caller: u64, generation: u64) -> bool {
        if !self.surface.render_allowed(caller, generation) || !self.present_pending {
            return false;
        }
        self.present_pending = false;
        true
    }

    fn adopt(&mut self, next: SurfaceLifecycle) {
        self.surface = next;
        self.present_pending = false;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    Down,
    Up,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: String,
    pub phase: KeyPhase,
    pub modifiers: Modifiers,
    pub repeat: bool,
    /// The host input method owns this key while a preedit is active.
    /// 输入法组合态期间由宿主输入法拥有该键。
    pub composing: bool,
}

// The manual equality below is intentionally part of the verified kernel and is exactly
// equivalent to the derived `u64` equality; keep the proof-bound representation unchanged.
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash)]
pub struct PointerId(pub u64);
use core::cmp::Ordering;

// Explicit comparison keeps the original u64 identity and total ordering.
// 显式比较保持原有 u64 身份及全序语义。
impl PartialEq for PointerId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for PointerId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0 < other.0 {
            Some(Ordering::Less)
        } else if self.0 > other.0 {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}
impl Ord for PointerId {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.0 < other.0 {
            Ordering::Less
        } else if self.0 > other.0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
    #[default]
    Unknown,
}

/// Pointer lifecycle phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
    #[default]
    Hover,
}

/// Mouse/pen button identity. Touch uses `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}


#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointerEvent {
    pub pointer_id: PointerId,
    pub kind: PointerKind,
    pub phase: PointerPhase,
    pub position: Point,
    pub pressure: f32,
    pub tilt: Point,
    pub twist: f32,
    pub contact: Size,
    pub is_primary: bool,
    pub eraser: bool,
    pub button: Option<MouseButton>,
    pub modifiers: Modifiers,
    pub timestamp_millis: u64,
}


#[derive(Clone, Debug, Default)]
pub struct PointerTracker {
    active: BTreeMap<PointerId, PointerEvent>,
}
use alloc::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Container,
    Text,
    Button,
    TextField,
    TextArea,
    Checkbox,
    Radio,
    Switch,
    Slider,
    Image,
    Link,
    List,
    ListItem,
    Table,
    TableRow,
    TableCell,
    Dialog,
    Popover,
    Tooltip,
    Menu,
    MenuItem,
    ScrollView,
    Unknown,
    /// Section heading; the level is 1..=6 by construction.
    /// 章节标题；级别由构造保证为 1..=6。
    Heading(HeadingLevel),
    /// Advisory status output (implicitly a polite live region).
    /// 状态信息（隐含礼貌型实时区域）。
    Status,
    /// Live region announcing its changes with the given politeness.
    /// 以指定礼貌级别播报变化的实时区域。
    Live(LivePoliteness),
    /// Important, time-sensitive message (implicitly an assertive live region).
    /// 重要且有时效的消息（隐含强制型实时区域）。
    Alert,
    /// Form landmark grouping input controls.
    /// 表单地标，组合输入控件。
    Form,
}

/// Heading level, always within 1..=6; build with `HeadingLevel::new`.
/// 标题级别，恒在 1..=6；经 `HeadingLevel::new` 构造。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingLevel(u8);

/// How assertively a live region's changes are announced.
/// 实时区域变化的播报强度。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LivePoliteness {
    Polite,
    Assertive,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusManager {
    order: Vec<NodeId>,
    focused: Option<NodeId>,
}

impl FocusManager {
    pub fn order(&self) -> &[NodeId] {
        &self.order
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    pub fn blur(&mut self) {
        self.focused = None;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusError {
    NotFocusable(NodeId),
}

impl FocusManager {
    pub fn focus(&mut self, node_id: NodeId) -> Result<(), FocusError> {
        let mut index = 0;
        while index < self.order.len() {
            if self.order[index].0 == node_id.0 {
                self.focused = Some(node_id);
                return Ok(());
            }
            index += 1;
        }
        Err(FocusError::NotFocusable(node_id))
    }
}

impl Role {
    /// Roles a keyboard Enter/Space should activate.
    pub fn is_activatable(self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::Checkbox
                | Role::Radio
                | Role::Switch
                | Role::Link
                | Role::MenuItem
        )
    }

    /// Space activates most controls, but not links (Space scrolls a page).
    pub fn activates_with_space(self) -> bool {
        matches!(
            self,
            Role::Button | Role::Checkbox | Role::Radio | Role::Switch | Role::MenuItem
        )
    }
}

fn no_command_modifier(event: &KeyEvent) -> bool {
    !event.modifiers.control && !event.modifiers.alt && !event.modifiers.meta
}

#[derive(Clone, Debug)]
pub struct KeyboardRouter {
    focus: FocusManager,
}

impl KeyboardRouter {
    pub fn focused(&self) -> Option<NodeId> {
        self.focus.focused()
    }
    pub fn blur(&mut self) {
        self.focus.blur();
    }

    /// Returns the previous focus and clears it, preserving traversal order.
    /// 取回原焦点并清除，保持遍历顺序；宿主可据此构造失焦意图。
    /// This does not identify an IME owner or deliver cancellation to an application.
    /// 此方法不代表输入法组合拥有者，也不负责向应用投递取消事件。
    pub fn take_focus(&mut self) -> Option<NodeId> {
        let previous = self.focus.focused();
        self.focus.blur();
        previous
    }
}

impl KeyboardRouter {
    /// Host-only close step: deny future admission before extracting focus.
    /// 宿主关闭步骤：先永久封闭准入，再取回失焦目标。
    /// The host must pair this gate with this router and gate every input entry.
    /// 宿主必须保证 gate 与路由器的归属匹配，并在所有输入入口检查准入。
    /// This returns an intent target; IME/pointer cancellation and delivery are separate.
    /// 返回值仅供构造失焦意图；预编辑、指针取消和外部投递另行处理。
    pub fn retire_focus(&mut self, gate: &mut ResourceGate) -> Option<NodeId> {
        gate.retire();
        self.take_focus()
    }
}

#[derive(Clone, Debug)]
struct Capture {
    target: Option<NodeId>,
    path: Vec<NodeId>,
}

/// Routes pointer events to scene nodes and maintains per-pointer capture.
#[derive(Clone, Debug, Default)]
pub struct PointerRouter {
    tracker: PointerTracker,
    captures: BTreeMap<PointerId, Capture>,
}

impl PointerRouter {
    pub fn has_capture(&self, pointer_id: PointerId) -> bool {
        self.captures.contains_key(&pointer_id)
    }

    /// Returns the original captured node, including when current hit testing differs.
    /// 返回捕获时的原目标，不按当前命中结果重新归属。
    /// None means absent capture or a capture that started outside all nodes.
    /// None 表示没有捕获或捕获起始于所有节点之外；用 has_capture 区分。
    pub fn captured_target(&self, pointer_id: PointerId) -> Option<NodeId> {
        if let Some(capture) = self.captures.get(&pointer_id) {
            capture.target
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverChange {
    Enter(NodeId),
    Leave(NodeId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HoverTracker {
    current: Option<NodeId>,
}

impl HoverTracker {
    pub fn hovered(&self) -> Option<NodeId> {
        self.current
    }

    /// Returns `Leave(old)` then `Enter(new)` when the target changes.
    pub fn update(&mut self, target: Option<NodeId>) -> Vec<HoverChange> {
        let unchanged = match (target, self.current) {
            (None, None) => true,
            (Some(next), Some(previous)) => next.0 == previous.0,
            _ => false,
        };
        if unchanged {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Some(old) = self.current {
            out.push(HoverChange::Leave(old));
        }
        if let Some(new) = target {
            out.push(HoverChange::Enter(new));
        }
        self.current = target;
        out
    }

    /// Clears hover, emitting a leave for the previous target.
    pub fn clear(&mut self) -> Vec<HoverChange> {
        self.update(None)
    }
}

impl HoverTracker {
    /// Host close: retire admission and return the real hover leave intent once.
    /// 宿主关闭：永久封闭准入，并取回真实悬停离开意图。
    /// The host pairs ownership and gates every input; delivery remains its responsibility.
    /// 宿主负责配对归属、所有输入入口准入检查和意图投递。
    /// This does not cancel pointer capture, presses, gestures or IME composition.
    /// 此方法不取消指针捕获、按压、手势或输入法组合。
    pub fn retire_hover(&mut self, gate: &mut ResourceGate) -> Vec<HoverChange> {
        gate.retire();
        self.clear()
    }
}

/// Pixel conversion failures for software presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelError {
    LengthMismatch,
}

/// Converts RGBA8 pixels to softbuffer's `0x00RRGGBB` words, ignoring alpha as before.
/// 将 RGBA8 转为 softbuffer 数值像素字；保持既有忽略 alpha 的行为。
pub fn rgba_to_u32(pixels: &[u8], out: &mut [u32]) -> Result<(), PixelError> {
    // Compare the ratio first so even the length check cannot overflow.
    // 先比较比例，确保长度检查本身也不会溢出；错误时尚未写入目标。
    if out.len() > pixels.len() / 4 || pixels.len() != out.len() * 4 {
        return Err(PixelError::LengthMismatch);
    }
    let mut index = 0usize;
    while index < out.len() {
        let base = index * 4;
        out[index] = rgb8_word(pixels[base], pixels[base + 1], pixels[base + 2]);
        index += 1;
    }
    Ok(())
}

/// Saturates a millisecond count instead of wrapping it into an earlier timestamp.
/// 毫秒数超出 u64 时饱和，不回绕成更早的时间戳。
pub fn desktop_timestamp_millis(elapsed: u128) -> u64 {
    if elapsed > u64::MAX as u128 {
        u64::MAX
    } else {
        elapsed as u64
    }
}

/// Total order of button identities; distinct buttons never share a key.
/// 按钮身份的全序键；不同按钮的键互不相同。
pub fn desktop_button_key(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(code) => 5 + code as u32,
    }
}

/// First index of ascending `held` whose key is not below `key`.
/// 升序 `held` 中首个键不小于 `key` 的下标。
pub fn desktop_button_position(held: &[MouseButton], key: u32) -> usize {
    let mut at = 0usize;
    while at < held.len() && desktop_button_key(held[at]) < key {
        at += 1;
    }
    at
}

/// A motion sample is a drag while any button is held, otherwise a hover.
pub fn desktop_motion_phase(any_pressed: bool) -> PointerPhase {
    if any_pressed {
        PointerPhase::Move
    } else {
        PointerPhase::Hover
    }
}

/// Assembles one mouse sample; the timestamp is an input, never read from a clock here.
/// 组装一个鼠标样本；时间戳由调用方传入，此处不读时钟。
pub fn desktop_pointer_event(
    pointer_id: PointerId,
    phase: PointerPhase,
    position: Point,
    button: Option<MouseButton>,
    pressure: f32,
    modifiers: Modifiers,
    timestamp_millis: u64,
) -> PointerEvent {
    PointerEvent {
        pointer_id,
        kind: PointerKind::Mouse,
        phase,
        position,
        pressure,
        tilt: Point { x: 0.0, y: 0.0 },
        twist: 0.0,
        contact: Size {
            width: 0.0,
            height: 0.0,
        },
        is_primary: matches!(button, Some(MouseButton::Left) | None),
        eraser: false,
        button,
        modifiers,
        timestamp_millis,
    }
}

pub struct DesktopPointerState {
    pointer_id: PointerId,
    position: Point,
    // Strictly ascending by `desktop_button_key`; a window holds only a handful of buttons.
    // 按 `desktop_button_key` 严格升序；一个窗口同时按下的按钮极少。
    pressed: Vec<MouseButton>,
    modifiers: Modifiers,
}

impl DesktopPointerState {
    pub fn new() -> Self {
        Self {
            pointer_id: PointerId(0),
            position: Point { x: 0.0, y: 0.0 },
            pressed: Vec::new(),
            modifiers: Modifiers {
                shift: false,
                control: false,
                alt: false,
                meta: false,
            },
        }
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    pub fn moved_at(&mut self, position: Point, timestamp_millis: u64) -> PointerEvent {
        self.position = position;
        let button = if self.pressed.is_empty() {
            None
        } else {
            Some(self.pressed[0])
        };
        desktop_pointer_event(
            self.pointer_id,
            desktop_motion_phase(!self.pressed.is_empty()),
            self.position,
            button,
            0.0,
            self.modifiers,
            timestamp_millis,
        )
    }

    pub fn pressed_at(&mut self, button: MouseButton, timestamp_millis: u64) -> PointerEvent {
        let key = desktop_button_key(button);
        let at = desktop_button_position(self.pressed.as_slice(), key);
        if at == self.pressed.len() || desktop_button_key(self.pressed[at]) != key {
            self.pressed.insert(at, button);
        }
        desktop_pointer_event(
            self.pointer_id,
            PointerPhase::Down,
            self.position,
            Some(button),
            1.0,
            self.modifiers,
            timestamp_millis,
        )
    }

    pub fn released_at(&mut self, button: MouseButton, timestamp_millis: u64) -> PointerEvent {
        let key = desktop_button_key(button);
        let at = desktop_button_position(self.pressed.as_slice(), key);
        if at < self.pressed.len() && desktop_button_key(self.pressed[at]) == key {
            self.pressed.remove(at);
        }
        desktop_pointer_event(
            self.pointer_id,
            PointerPhase::Up,
            self.position,
            Some(button),
            0.0,
            self.modifiers,
            timestamp_millis,
        )
    }
}

/// Opaque index and generation; the all-zero value is never live.
/// 不透明的下标与代数；全零值永远无效。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub index: u32,
    pub generation: u32,
}

/// What a handle means to its table: live, issued earlier and since released, or never
/// issued by this slot. Stale generations stay stale; see `verification.md`.
/// 句柄对其表的含义：存活、曾签发且已释放、或此槽位从未签发。过期代数永远过期。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleStatus {
    Live,
    Stale,
    Invalid,
}

/// Slot generations only move upward: odd is live, even is vacant, and the retired value
/// is permanently vacant, so generations never wrap.
/// 槽位代数只增不减：奇数存活、偶数空闲，退役值永久空闲，代数不回绕。
pub struct HandleAllocator {
    slots: Vec<u32>,
    free: Vec<u32>,
    capacity: u32,
    first_generation: u32,
}

/// Owned values addressed through an allocator; a value is reachable only by its live handle.
/// 经分配器寻址的自有值；只有存活句柄才能取到对应值。
pub struct HandleTable<T> {
    allocator: HandleAllocator,
    values: Vec<Option<T>>,
}

pub const HANDLE_GENERATION_RETIRED: u32 = 0xFFFF_FFFE;

fn handle_generation_is_live(generation: u32) -> bool {
    generation & 1 == 1
}

/// Reports allocation failure instead of aborting; the contents are untouched either way.
/// 分配失败时返回 false 而不中止进程；两种情况下内容都不变。
fn handle_try_reserve<U>(values: &mut Vec<U>, additional: usize) -> bool {
    values.try_reserve(additional).is_ok()
}

impl HandleAllocator {
    pub fn new(capacity: u32) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            capacity,
            first_generation: 1,
        }
    }

    /// A restarted host may start past the handles of its previous run; the first
    /// generation must be odd and below the retired value.
    /// 重启的宿主可越过上次运行签发的句柄；首代必须为奇数且低于退役值。
    pub fn with_first_generation(capacity: u32, first_generation: u32) -> Option<Self> {
        if !handle_generation_is_live(first_generation)
            || first_generation >= HANDLE_GENERATION_RETIRED
        {
            return None;
        }
        Some(Self {
            slots: Vec::new(),
            free: Vec::new(),
            capacity,
            first_generation,
        })
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn is_live(&self, handle: Handle) -> bool {
        let index = handle.index as usize;
        index < self.slots.len()
            && handle_generation_is_live(handle.generation)
            && self.slots[index] == handle.generation
    }

    /// A generation odd and below the slot's current one was issued earlier, perhaps by a
    /// previous run that `with_first_generation` skipped past.
    /// 低于槽位当前代数的奇数代曾被签发，也可能来自 `with_first_generation` 越过的上次运行。
    pub fn status(&self, handle: Handle) -> HandleStatus {
        let index = handle.index as usize;
        if index >= self.slots.len() || !handle_generation_is_live(handle.generation) {
            return HandleStatus::Invalid;
        }
        let current = self.slots[index];
        if handle.generation == current {
            HandleStatus::Live
        } else if handle.generation < current {
            HandleStatus::Stale
        } else {
            HandleStatus::Invalid
        }
    }

    /// Reuses a vacant slot under a newer generation or appends one within capacity.
    /// `None` means no slot can be issued: full, or storage could not grow.
    /// 以更新的代数复用空闲槽位，或在容量内追加；已满或存储无法增长时返回 `None`，且不改变状态。
    pub fn allocate(&mut self) -> Option<Handle> {
        match self.free.pop() {
            Some(index) => {
                let generation = self.slots[index as usize] + 1;
                self.slots[index as usize] = generation;
                Some(Handle { index, generation })
            }
            None => {
                if self.slots.len() >= self.capacity as usize {
                    return None;
                }
                // The free list is empty here, so this room covers every later release.
                // 此处空闲表为空，这份空间足以容纳此后的每次释放，释放因此从不分配。
                if !handle_try_reserve(&mut self.slots, 1)
                    || !handle_try_reserve(&mut self.free, self.slots.len() + 1)
                {
                    return None;
                }
                let index = self.slots.len() as u32;
                let generation = self.first_generation;
                self.slots.push(generation);
                Some(Handle { index, generation })
            }
        }
    }

    /// Stale, foreign and repeated releases are rejected without any state change.
    /// 过期、外来与重复释放一律拒绝且不改变状态；代数耗尽的槽位退役，不再复用。
    pub fn release(&mut self, handle: Handle) -> bool {
        if !self.is_live(handle) {
            return false;
        }
        let generation = handle.generation + 1;
        self.slots[handle.index as usize] = generation;
        if generation != HANDLE_GENERATION_RETIRED {
            self.free.push(handle.index);
        }
        true
    }
}

impl<T> HandleTable<T> {
    pub fn new(capacity: u32) -> Self {
        Self {
            allocator: HandleAllocator::new(capacity),
            values: Vec::new(),
        }
    }

    pub fn with_first_generation(capacity: u32, first_generation: u32) -> Option<Self> {
        match HandleAllocator::with_first_generation(capacity, first_generation) {
            None => None,
            Some(allocator) => Some(Self {
                allocator,
                values: Vec::new(),
            }),
        }
    }

    pub fn is_live(&self, handle: Handle) -> bool {
        self.allocator.is_live(handle)
    }

    pub fn status(&self, handle: Handle) -> HandleStatus {
        self.allocator.status(handle)
    }

    /// A table that is full or cannot grow returns the value to the caller instead of dropping it.
    /// 表满或无法增长时把值交还调用方，不丢弃。
    pub fn insert(&mut self, value: T) -> Result<Handle, T> {
        // Reserved before the allocator commits, so failure leaves no live slot without a value.
        // 先于分配器提交预留，失败时不会留下无值的存活槽位。
        if self.allocator.free.is_empty()
            && self.allocator.slots.len() < self.allocator.capacity as usize
            && !handle_try_reserve(&mut self.values, 1)
        {
            return Err(value);
        }
        match self.allocator.allocate() {
            None => Err(value),
            Some(handle) => {
                let index = handle.index as usize;
                if index == self.values.len() {
                    self.values.push(Some(value));
                } else {
                    self.values[index] = Some(value);
                }
                Ok(handle)
            }
        }
    }

    pub fn get(&self, handle: Handle) -> Option<&T> {
        if !self.allocator.is_live(handle) {
            return None;
        }
        self.values[handle.index as usize].as_ref()
    }

    pub fn get_mut(&mut self, handle: Handle) -> Option<&mut T> {
        if !self.allocator.is_live(handle) {
            return None;
        }
        self.values[handle.index as usize].as_mut()
    }

    pub fn remove(&mut self, handle: Handle) -> Option<T> {
        if !self.allocator.release(handle) {
            return None;
        }
        self.values[handle.index as usize].take()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

/// Computes the next version without wrapping or saturating.
/// 计算下一版本，不回绕、不饱和；证明脚本直接提取此函数体。
fn next_scene_version(version: u64) -> Option<u64> {
    if version < u64::MAX {
        Some(version + 1)
    } else {
        None
    }
}

enum VersionDecision {
    Stale,
    Unchanged,
    Exhausted,
    Advance(u64),
}

/// Gives stale versions priority, preserves empty batches and rejects exhaustion.
/// 优先拒绝陈旧版本，空批保持版本，非空批耗尽时拒绝；不修改任何场景状态。
fn decide_scene_version(current: u64, expected: u64, empty: bool) -> VersionDecision {
    if expected != current {
        VersionDecision::Stale
    } else if empty {
        VersionDecision::Unchanged
    } else {
        match next_scene_version(current) {
            Some(next) => VersionDecision::Advance(next),
            None => VersionDecision::Exhausted,
        }
    }
}

/// Folds the patches over a detached value in order, each exactly once, stopping at the first
/// failure. The value moves through the fold, so a failure leaves nothing to undo or to reuse.
/// 按序把补丁折叠到独立值上，每个恰好一次，遇首个失败即停；值随折叠移动，失败后无可撤销也无可复用。
fn stage_scene_batch<S, P, E>(
    detached: S,
    patches: &[P],
    apply: impl Fn(S, &P) -> Result<S, E>,
) -> Result<S, E> {
    let mut staged = detached;
    let mut index = 0usize;
    while index < patches.len() {
        match apply(staged, &patches[index]) {
            Ok(next) => staged = next,
            Err(error) => return Err(error),
        }
        index += 1;
    }
    Ok(staged)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub root: Node,
    version: u64,
}

/// Decides the version, checks, stages and commits. Both closures only read the active scene, so
/// the single write is the whole replacement at the end, carrying the decided version.
/// 判定版本、检查、暂存、提交。两个闭包只能读活动场景，唯一的写入是末尾的整体替换，版本取判定结果。
fn commit_scene_batch(
    scene: &mut Scene,
    expected_version: u64,
    empty: bool,
    validate: impl FnOnce(&Scene) -> Result<(), SceneError>,
    stage: impl FnOnce(&Scene) -> Result<Scene, SceneError>,
) -> Result<(), SceneError> {
    let next_version = match decide_scene_version(scene.version, expected_version, empty) {
        VersionDecision::Stale => {
            return Err(SceneError::StaleVersion {
                expected: expected_version,
                actual: scene.version,
            })
        }
        VersionDecision::Unchanged => return validate(&*scene),
        VersionDecision::Exhausted => return Err(SceneError::VersionExhausted),
        VersionDecision::Advance(version) => version,
    };
    let checked = validate(&*scene);
    match checked {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let staged = stage(&*scene);
    match staged {
        Ok(staged) => {
            let mut next = staged;
            next.version = next_version;
            *scene = next;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn scene_move_failure(
    root_matches: bool,
    id_present: bool,
    parent_present: bool,
    parent_in_moved: bool,
    id: NodeId,
    parent: NodeId,
) -> Option<SceneError> {
    if root_matches {
        Some(SceneError::CannotMoveRoot)
    } else if !id_present {
        Some(SceneError::NodeNotFound(id))
    } else if !parent_present {
        Some(SceneError::NodeNotFound(parent))
    } else if parent_in_moved {
        Some(SceneError::MoveIntoDescendant(id))
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Container,
    Text(String),
    Rect,
    Canvas(Canvas),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub style: Style,
    pub semantics: Semantics,
    /// Child offset applied after layout; scroll containers update this.
    pub scroll: Point,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScenePatch {
    Insert {
        parent: NodeId,
        index: usize,
        node: Node,
    },
    UpdateStyle {
        id: NodeId,
        style: Style,
    },
    /// Framework-owned semantic state; native backends may require stable component role.
    UpdateSemantics {
        id: NodeId,
        semantics: Semantics,
    },
    UpdateText {
        id: NodeId,
        text: String,
    },
    SetCanvas {
        id: NodeId,
        canvas: Canvas,
    },
    SetScroll {
        id: NodeId,
        offset: Point,
    },
    /// Moves an existing node without changing its identity or interaction state.
    Move {
        id: NodeId,
        parent: NodeId,
        index: usize,
    },
    Remove {
        id: NodeId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    NodeNotFound(NodeId),
    DuplicateNode(NodeId),
    CannotRemoveRoot,
    CannotMoveRoot,
    MoveIntoDescendant(NodeId),
    NotText(NodeId),
    NotCanvas(NodeId),
    StaleVersion {
        expected: u64,
        actual: u64,
    },
    /// No fresh version is available; the scene remains unchanged.
    /// 无可用的新版本；场景保持不变。
    VersionExhausted,
}

/// Inserts a node under the first matching parent, clamping the index to the child count.
fn insert_node_at(root: &mut Node, parent: NodeId, index: usize, node: Node) -> bool {
    let Some(parent_node) = find_node_mut(root, parent) else {
        return false;
    };
    let len = parent_node.children.len();
    let index = if index < len { index } else { len };
    parent_node.children.insert(index, node);
    true
}

/// Takes the first direct child with the identity; failing that, takes it from the first child
/// subtree that holds it. A miss changes nothing.
fn take_node(parent: &mut Node, id: NodeId) -> Option<Node> {
    let mut index = 0usize;
    while index < parent.children.len() {
        if parent.children[index].id.0 == id.0 {
            return Some(parent.children.remove(index));
        }
        index += 1;
    }
    index = 0;
    while index < parent.children.len() {
        let found = find_node(&parent.children[index], id);
        if found.is_some() {
            return take_node(&mut parent.children[index], id);
        }
        index += 1;
    }
    None
}

/// Removes the first direct child with the identity; failing that, removes it from the first child
/// subtree that holds it, leaving every other node alone. A miss changes nothing.
/// 优先移除首个身份匹配的直接子节点；否则只在首个含该身份的子树中移除，其余节点不动；未命中不做任何改变。
fn remove_node(parent: &mut Node, id: NodeId) -> bool {
    let mut index = 0usize;
    while index < parent.children.len() {
        if parent.children[index].id.0 == id.0 {
            parent.children.remove(index);
            return true;
        }
        index += 1;
    }
    index = 0;
    while index < parent.children.len() {
        if remove_node(&mut parent.children[index], id) {
            return true;
        }
        index += 1;
    }
    false
}

/// Admits, extracts and reinserts one existing subtree. The extracted value is moved into the
/// destination, so the successful result is exactly removal followed by insertion.
fn move_node_at(
    root: &mut Node,
    id: NodeId,
    parent: NodeId,
    index: usize,
) -> Result<(), SceneError> {
    let moved = find_node(&*root, id);
    let parent_present = find_node(&*root, parent).is_some();
    let parent_in_moved = match moved {
        Some(moved) => find_node(moved, parent).is_some(),
        None => false,
    };
    let _source_match = moved;
    match scene_move_failure(
        root.id.0 == id.0,
        moved.is_some(),
        parent_present,
        parent_in_moved,
        id,
        parent,
    ) {
        Some(failure) => return Err(failure),
        None => {}
    }
    let Some(moved) = take_node(root, id) else {
        return Err(SceneError::NodeNotFound(id));
    };
    let inserted = insert_node_at(root, parent, index, moved);
    if inserted {
        Ok(())
    } else {
        Err(SceneError::NodeNotFound(parent))
    }
}

/// Finds the first node with the identity in preorder, the node itself before its children.
/// 按前序查找首个身份匹配的节点，节点本身先于其子节点。
fn find_node(node: &Node, id: NodeId) -> Option<&Node> {
    if node.id.0 == id.0 {
        return Some(node);
    }
    let mut index = 0usize;
    while index < node.children.len() {
        if let Some(found) = find_node(&node.children[index], id) {
            return Some(found);
        }
        index += 1;
    }
    None
}

/// Lends the node `find_node` finds. Each level asks the read path which child holds the identity,
/// because a mutable borrow cannot be retried after a miss; a level rescans, so the worst case is
/// nodes times depth. Nothing is written here: the only change to the tree is what the caller
/// does through the returned reference.
/// 借出 `find_node` 找到的节点。每层先用只读路径问清哪个子节点含该身份，因为可变借用在未命中后无法重试；
/// 每层都重新扫描，最坏为节点数乘深度。这里不写任何东西：树的唯一变化是调用者经返回引用所做的修改。
fn find_node_mut(node: &mut Node, id: NodeId) -> Option<&mut Node> {
    if node.id.0 == id.0 {
        return Some(node);
    }
    let mut index = 0usize;
    while index < node.children.len() {
        if find_node(&node.children[index], id).is_some() {
            return find_node_mut(&mut node.children[index], id);
        }
        index += 1;
    }
    None
}

/// Applies one patch to the tree. The tree is the only state reached; a failed patch may be left
/// half-way by nothing: each branch writes only after its checks pass.
/// 把一个补丁应用到树。只触及树本身；每个分支都先通过检查再写入。
fn apply_tree_patch(root: &mut Node, patch: ScenePatch) -> Result<(), SceneError> {
    match patch {
        ScenePatch::Insert {
            parent,
            index,
            node,
        } => {
            if insert_node_at(root, parent, index, node) {
                Ok(())
            } else {
                Err(SceneError::NodeNotFound(parent))
            }
        }
        ScenePatch::UpdateSemantics { id, semantics } => match find_node_mut(root, id) {
            Some(node) => {
                node.semantics = semantics;
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::UpdateStyle { id, style } => match find_node_mut(root, id) {
            Some(node) => {
                node.style = style;
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::SetScroll { id, offset } => match find_node_mut(root, id) {
            Some(node) => {
                node.scroll = offset;
                Ok(())
            }
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::Move { id, parent, index } => {
            move_node_at(root, id, parent, index)
        },
        ScenePatch::UpdateText { id, text } => match find_node_mut(root, id) {
            Some(node) => match &mut node.kind {
                NodeKind::Text(existing) => {
                    *existing = text;
                    Ok(())
                }
                _ => Err(SceneError::NotText(id)),
            },
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::SetCanvas { id, canvas } => match find_node_mut(root, id) {
            Some(node) => match &mut node.kind {
                NodeKind::Canvas(existing) => {
                    *existing = canvas;
                    Ok(())
                }
                _ => Err(SceneError::NotCanvas(id)),
            },
            None => Err(SceneError::NodeNotFound(id)),
        },
        ScenePatch::Remove { id } => {
            if root.id.0 == id.0 {
                return Err(SceneError::CannotRemoveRoot);
            }
            if remove_node(root, id) {
                Ok(())
            } else {
                Err(SceneError::NodeNotFound(id))
            }
        }
    }
}

/// Validates all descendants of an insertion before any follow-up patch can observe ambiguous
/// identities, then applies the patch. The set starts empty and is local to staging.
/// 插入前检查全部后代，避免后续补丁观察到歧义身份，再应用补丁；集合从空开始，仅在暂存阶段使用。
fn apply_checked_patch<T: IdentitySet>(
    root: &mut Node,
    patch: ScenePatch,
    ids: &mut T,
) -> Result<(), SceneError> {
    if let ScenePatch::Insert { node, .. } = &patch {
        match check_insert_identities(&*root, node, ids) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
    }
    apply_tree_patch(root, patch)
}

/// A set that can start empty, so each check begins from nothing.
/// 可从空开始的集合，每次检查都从零开始。
trait EmptyIdentitySet: IdentitySet {
    fn empty() -> Self;
}

/// Rechecks the whole tree from an empty set.
/// 从空集合开始重查全树。
fn validate_scene_ids<T: EmptyIdentitySet>(scene: &Scene) -> Result<(), SceneError> {
    let mut ids = T::empty();
    collect_unique_ids(&scene.root, &mut ids)
}

/// A plain function with no receiver, so staging cannot reach the active scene.
/// 无接收者的普通函数，暂存阶段无法触及活动场景。
fn apply_detached_patch<T: EmptyIdentitySet>(
    mut staged: Scene,
    patch: &ScenePatch,
) -> Result<Scene, SceneError> {
    let mut ids = T::empty();
    match apply_checked_patch(&mut staged.root, patch.clone(), &mut ids) {
        Ok(()) => Ok(staged),
        Err(error) => Err(error),
    }
}

/// The whole batch: the tree is checked first, each patch keeps it free of repeats, and only a
/// complete fold replaces the active scene.
/// 整批：先查全树，每个补丁都保持无重复，只有完整折叠才替换活动场景。
fn apply_unique_batch<T: EmptyIdentitySet>(
    scene: &mut Scene,
    expected_version: u64,
    patches: &[ScenePatch],
) -> Result<(), SceneError> {
    commit_scene_batch(
        scene,
        expected_version,
        patches.is_empty(),
        validate_scene_ids::<T>,
        |active: &Scene| stage_scene_batch(active.clone(), patches, apply_detached_patch::<T>),
    )
}

/// What the identity traversal needs from its set: records an identity and reports whether it was new.
/// 身份遍历对集合的全部要求：记录一个身份，并报告它此前是否未出现。
trait IdentitySet {
    fn admit(&mut self, id: NodeId) -> bool;
}

/// Traverses iteratively so identity validation itself does not recurse.
/// 迭代遍历，身份校验本身不递归；不代表后续布局/克隆已具备深度资源限制。
fn collect_unique_ids<T: IdentitySet>(root: &Node, ids: &mut T) -> Result<(), SceneError> {
    let mut pending = Vec::new();
    pending.push(root);
    loop {
        let node = match pending.pop() {
            Some(node) => node,
            None => return Ok(()),
        };
        if !ids.admit(node.id) {
            return Err(SceneError::DuplicateNode(node.id));
        }
        let mut index = node.children.len();
        while index > 0 {
            index -= 1;
            pending.push(&node.children[index]);
        }
    }
}

/// The identity check for an insertion: the tree first, then the inserted subtree, in one set.
/// 插入的身份检查：先查树，再查待插入子树，共用一个集合。
fn check_insert_identities<T: IdentitySet>(
    root: &Node,
    inserted: &Node,
    ids: &mut T,
) -> Result<(), SceneError> {
    match collect_unique_ids(root, ids) {
        Ok(()) => collect_unique_ids(inserted, ids),
        Err(error) => Err(error),
    }
}

// Extracted so the harnesses run the real set; that it obeys the contract is tested, not proved.
// 提取出来让验证运行真实集合；它是否满足契约只经测试，未证明。
impl IdentitySet for BTreeSet<NodeId> {
    fn admit(&mut self, id: NodeId) -> bool {
        self.insert(id)
    }
}

impl EmptyIdentitySet for BTreeSet<NodeId> {
    fn empty() -> Self {
        BTreeSet::new()
    }
}
