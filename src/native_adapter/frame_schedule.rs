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

// BEGIN VERIFIED FRAME DAMAGE TYPE
/// Damage of a produced frame relative to the previous frame this view produced.
/// 本视图产出帧相对其上一次产出帧的损伤。
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FrameDamage {
    /// Pixel-identical to the previous frame (including a reused cached frame).
    /// 与上一帧逐像素一致（含复用缓存帧）。
    Unchanged,
    /// No comparable previous frame: first frame, extent change or host invalidation.
    /// 无可比较的上一帧：首帧、尺寸变化或宿主失效。
    Full,
    /// Smallest rectangle covering every changed pixel.
    /// 覆盖全部变化像素的最小矩形。
    Rect {x:u32,y:u32,width:u32,height:u32},
}
// END VERIFIED FRAME DAMAGE TYPE

// BEGIN VERIFIED DAMAGE SPAN KERNEL
/// Smallest inclusive span covering both the current span (if any) and `at`.
/// 覆盖当前区间（若有）与 `at` 的最小闭区间。
pub fn damage_span_extend(span: Option<(u32, u32)>, at: u32) -> (u32, u32) {
    match span {
        None => (at, at),
        Some((lo, hi)) => (if at < lo { at } else { lo }, if at > hi { at } else { hi }),
    }
}
// END VERIFIED DAMAGE SPAN KERNEL

// BEGIN VERIFIED FRAME DAMAGE KERNEL
/// Exact damage between two packed RGBA8 frames of the same extent; `previous` is None when
/// no comparable frame exists. Rows are compared whole, then scanned from both ends.
/// 同尺寸紧凑 RGBA8 两帧间的精确损伤；无可比较上一帧时 `previous` 为 None。
pub fn frame_damage(previous:Option<(u32,u32,&[u8])>,width:u32,height:u32,next:&[u8])->FrameDamage {
    let Some((previous_width,previous_height,previous))=previous else {return FrameDamage::Full;};
    let row=width as usize*4;
    if (previous_width,previous_height)!=(width,height) || row==0 || previous.len()!=next.len()
        || next.len()!=row*height as usize {return FrameDamage::Full;}
    let (mut xs,mut ys)=(None,None);
    for (y,(before,after)) in previous.chunks_exact(row).zip(next.chunks_exact(row)).enumerate() {
        if before==after {continue;}
        let differs=|x:&usize|before[x*4..x*4+4]!=after[x*4..x*4+4];
        let first=(0..width as usize).find(differs).unwrap_or(0);
        let last=(0..width as usize).rev().find(differs).unwrap_or(first);
        xs=Some(damage_span_extend(Some(damage_span_extend(xs,first as u32)),last as u32));
        ys=Some(damage_span_extend(ys,y as u32));
    }
    match (xs,ys) {
        (Some((x0,x1)),Some((y0,y1)))=>FrameDamage::Rect {x:x0,y:y0,width:x1-x0+1,height:y1-y0+1},
        _=>FrameDamage::Unchanged,
    }
}
// END VERIFIED FRAME DAMAGE KERNEL
