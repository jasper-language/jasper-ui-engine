use vstd::prelude::*;
verus! {

/// A fill is admitted only when nonempty and wholly inside the node; it is rejected, never clipped.
/// 填充只有非空且完全落在节点内才准入；越界拒绝，不裁剪。
pub fn paint_rect_admits(x: u32, y: u32, width: u32, height: u32, node_width: u32, node_height: u32) -> (result: bool)
    ensures result == (width > 0 && height > 0
        && x as int + width as int <= node_width as int
        && y as int + height as int <= node_height as int),
{
    width <= node_width
        && x <= node_width - width
        && height <= node_height
        && y <= node_height - height
}

}
