//! Numeric admission for the pinned CPU renderer, not a bound on backend scratch allocations.
// BEGIN VERIFIED CPU FRAME BYTES KERNEL
pub(crate) fn cpu_frame_bytes(width:u32,height:u32,budget:usize)->Option<usize> {
    if width==0 || height==0 || width>65535 || height>65535 {return None;}
    let bytes=(width as u64)*(height as u64)*4;
    // AnyRender 0.14 computes packed size in u32 before allocating.
    if bytes>budget as u64 || bytes>4294967295 {return None;}
    Some(bytes as usize)
}
// END VERIFIED CPU FRAME BYTES KERNEL
