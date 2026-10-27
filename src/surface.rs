//! Allocation-free software surface admission helpers for all UI hosts.
//! 面向所有 UI 宿主的软件 Surface 准入函数，不分配内存且不依赖窗口或 GPU。

// BEGIN VERIFIED SURFACE ROW KERNEL
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
// END VERIFIED SURFACE ROW KERNEL

// BEGIN VERIFIED SURFACE SPAN KERNEL
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
// END VERIFIED SURFACE SPAN KERNEL

// BEGIN VERIFIED SURFACE BORROW KERNEL
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
// END VERIFIED SURFACE BORROW KERNEL

// BEGIN VERIFIED SURFACE WRITE KERNEL
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
// END VERIFIED SURFACE WRITE KERNEL

// BEGIN VERIFIED SURFACE VIEW KERNEL
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
// END VERIFIED SURFACE VIEW KERNEL

// BEGIN VERIFIED PIXEL PACK KERNEL
/// Packs RGB8 channels as a numeric 0x00RRGGBB software-presentation word.
/// 将 RGB8 通道编码为数值 0x00RRGGBB，供软件呈现宿主使用。
/// This does not blend alpha or prescribe the host's native byte order.
/// 不做 alpha 混色，也不规定宿主的原生字节序。
pub fn rgb8_word(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 * 65536 + green as u32 * 256 + blue as u32
}
// END VERIFIED PIXEL PACK KERNEL
