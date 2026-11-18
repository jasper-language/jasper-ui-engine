use vstd::prelude::*;
verus! {


pub(crate) fn core_frame_bytes(width: u32, height: u32) -> (result: Option<u64>)
    ensures match result {
        Some(bytes) => 0 < width <= 65535 && 0 < height <= 65535
            && bytes as int == width as int * height as int * 4,
        None => width == 0 || height == 0 || width > 65535 || height > 65535,
    },
{
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return None;
    }
    assert(width as int * height as int * 4 <= 65535 * 65535 * 4) by (nonlinear_arith)
        requires 0 < width as int <= 65535, 0 < height as int <= 65535;
    Some(width as u64 * height as u64 * 4)
}

pub(crate) fn admit_core_frame_bytes(
    width: u32,
    height: u32,
    max_bytes: usize,
) -> (result: Option<u64>)
    ensures match result {
        Some(bytes) => 0 < width <= 65535 && 0 < height <= 65535
            && bytes as int == width as int * height as int * 4
            && bytes <= max_bytes as u64,
        None => width == 0 || height == 0 || width > 65535 || height > 65535
            || width as int * height as int * 4 > max_bytes as int,
    },
{
    let bytes = core_frame_bytes(width, height)?;
    if false {
        None
    } else {
        Some(bytes)
    }
}

}
