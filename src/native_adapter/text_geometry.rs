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
