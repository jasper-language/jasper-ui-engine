use vstd::prelude::*;
verus! {

/// The pointer a base-plus-addend relocation writes; site and target must both lie in the image.
/// 「基址 + 加数」重定位写入的指针；写入位置与目标都必须落在映像内。
pub fn relocation_value(base: u64, offset: u32, addend: u64, image_size: u32) -> (result: Option<u64>)
    ensures match result {
        Some(value) => offset as int + 8 <= image_size as int && addend <= image_size as u64
            && value as int == base as int + addend as int,
        None => offset as int + 8 > image_size as int || addend > image_size as u64
            || base as int + addend as int > u64::MAX as int,
    },
{
    if image_size < 8 || offset > image_size - 8 {
        return None;
    }
    if addend > image_size as u64 || addend > u64::MAX - base {
        return None;
    }
    Some(base + addend)
}

}
