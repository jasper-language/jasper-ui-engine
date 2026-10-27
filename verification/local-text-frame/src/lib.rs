//! Only the scalar storage admission slice; not the complete text renderer.
//! 仅验证标量存储准入切片，不代表完整文本渲染器。
include!(concat!(env!("OUT_DIR"),"/actual.rs"));
pub fn check(width:u32,height:u32,budget:usize){
    let expected=if width==0||height==0||width>i32::MAX as u32||height>i32::MAX as u32 {None} else {
        let row=width as u128*4;let end=row*height as u128;
        if end>budget as u128 {None} else {Some((row as usize,end as usize))}
    };
    assert_eq!(text_frame_storage(width,height,budget),expected);
}
#[cfg(test)] mod tests{
use super::*;
#[test] fn miri_boundaries(){for (w,h,b) in [(0,1,usize::MAX),(1,0,usize::MAX),(1,1,3),(1,1,4),(520,420,520*420*4),(u32::MAX,1,usize::MAX),(i32::MAX as u32,i32::MAX as u32,usize::MAX)]{check(w,h,b);}}
use proptest::prelude::*;
proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
#[test] fn arbitrary_dimensions_and_budget(w in any::<u32>(),h in any::<u32>(),b in any::<usize>()){check(w,h,b);}
#[test] fn valid_and_near_budget(w in 1u32..4096,h in 1u32..4096,under in any::<bool>()){let end=w as usize*h as usize*4;check(w,h,end-usize::from(under));}}
}
#[cfg(kani)] mod proof {use super::*;#[kani::proof]fn text_storage(){check(kani::any(),kani::any(),kani::any());}}

/// Independent integer-domain oracle for the actual native-position boundary.
/// 实际原生位置边界的独立整数域参考性质。
pub fn check_hit(position:i32,len:usize){
    let expected=if position as i128>=0 && position as i128<=len as i128 {Some(position as usize)} else {None};
    assert_eq!(text_hit_offset(position,len),expected);
}
#[cfg(test)] mod hit_tests {use super::*;use proptest::prelude::*;
#[test] fn miri_hit_boundaries(){for (position,len) in [(i32::MIN,usize::MAX),(-1,usize::MAX),(0,0),(1,0),(1,1),(i32::MAX,i32::MAX as usize),(i32::MAX,0)]{check_hit(position,len);}}
proptest! {#![proptest_config(ProptestConfig::with_cases(256))]
#[test] fn hit_offset_property(position in any::<i32>(),len in any::<usize>()){check_hit(position,len);}}
}
#[cfg(kani)] mod hit_proof {use super::*;#[kani::proof]fn text_hit_position(){check_hit(kani::any(),kani::any());}}
