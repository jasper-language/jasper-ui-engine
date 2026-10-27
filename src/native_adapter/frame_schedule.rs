//! On-demand frame admission: a cached frame replaces a repaint only when provably current.
//! 按需帧准入：仅当缓存帧确属当前状态时才以其代替重绘。
// BEGIN VERIFIED FRAME REUSE KERNEL
/// True only when nothing was invalidated since the cached frame was painted and the cache
/// has exactly the current extent and the admitted packed length.
/// 仅当缓存帧绘制后无任何失效，且其尺寸与当前一致、长度等于已准入的紧凑长度时为真。
pub fn frame_reuse_admits(dirty: bool, cached_width: u32, cached_height: u32, cached_len: usize,
    width: u32, height: u32, expected: usize) -> bool {
    !dirty && cached_width == width && cached_height == height && cached_len == expected
}
// END VERIFIED FRAME REUSE KERNEL
