//! Source-text offset admission for native text hit/caret geometry.
//! 原生文字命中/光标几何的源文本偏移准入。

// BEGIN VERIFIED TEXT OFFSET KERNEL
/// A native layout offset is a source-text UTF-8 offset only when the layout holds exactly
/// the source text (same length; content equality is checked by the caller) and the offset is
/// a character boundary inside it, end included.
/// 仅当布局文本与源文本一致（长度相同；内容相等由调用方检查）且偏移为其中的字符边界（含末尾）时，
/// 原生布局偏移才是源文本 UTF-8 偏移。
pub fn text_offset_admits(layout_len: usize, source_len: usize, offset: usize, on_boundary: bool) -> bool {
    layout_len == source_len && offset <= source_len && on_boundary
}
// END VERIFIED TEXT OFFSET KERNEL

// BEGIN VERIFIED TEXT RANGE KERNEL
/// A native layout range is a source-text UTF-8 range only when the layout holds exactly the
/// source text (same length; content equality is checked by the caller), start ≤ end ≤ the
/// source length, and both ends are character boundaries.
/// 仅当布局文本与源文本一致（长度相同；内容相等由调用方检查）、start ≤ end ≤ 源文本长度且两端均为
/// 字符边界时，原生布局区间才是源文本 UTF-8 区间。
pub fn text_range_admits(layout_len: usize, source_len: usize, start: usize, end: usize, start_on_boundary: bool, end_on_boundary: bool) -> bool {
    layout_len == source_len && start <= end && end <= source_len && start_on_boundary && end_on_boundary
}
// END VERIFIED TEXT RANGE KERNEL
