//! Exact production native backend frame-import bodies, not a replacement backend.
extern crate self as jasper_ui_engine;
pub struct UiBatch;
include!(concat!(env!("OUT_DIR"),"/actual.rs"));
mod cpu_budget {include!(concat!(env!("OUT_DIR"),"/cpu_budget.rs"));}
pub(crate) mod backend {pub(crate) use crate::{BackendError,RenderBackend,submit_native_frame};}
pub(crate) mod surface {pub(crate) use crate::{rgba8_row_bytes,surface_span,write_surface_row};}

#[derive(Clone,Debug)]
struct RecordingBackend {
 submit_ok:bool,present_ok:bool,submit_calls:u8,present_calls:u8,
 width:u32,height:u32,pixels:Vec<u8>,
}
impl RenderBackend for RecordingBackend {
 fn resize(&mut self,_:Size) {}
 fn submit(&mut self,_:&UiBatch)->Result<(),BackendError>{Ok(())}
 fn submit_rgba8(&mut self,width:u32,height:u32,pixels:Vec<u8>)->Result<(),BackendError>{
  self.submit_calls+=1;self.width=width;self.height=height;self.pixels=pixels;
  if self.submit_ok {Ok(())} else {Err(BackendError::Invalid("submit".into()))}
 }
 fn present(&mut self)->Result<(),BackendError>{
  self.present_calls+=1;
  if self.present_ok {Ok(())} else {Err(BackendError::Io("present".into()))}
 }
}
struct DefaultBackend {present_calls:u8}
impl RenderBackend for DefaultBackend {
 fn resize(&mut self,_:Size) {}
 fn submit(&mut self,_:&UiBatch)->Result<(),BackendError>{Ok(())}
 fn present(&mut self)->Result<(),BackendError>{self.present_calls+=1;Ok(())}
}
/// Runtime property over the exact production trait default and submission kernel.
pub fn check_submission(width:u32,height:u32,bytes:&[u8],submit_ok:bool,present_ok:bool)->bool {
 let mut backend=RecordingBackend {submit_ok,present_ok,submit_calls:0,present_calls:0,width:0,height:0,pixels:Vec::new()};
 let result=submit_native_frame(&mut backend,width,height,bytes.to_vec());
 let outcome=match result {
  Ok(())=>submit_ok && present_ok,
  Err(BackendError::Invalid(_))=>!submit_ok,
  Err(BackendError::Io(_))=>submit_ok && !present_ok,
  Err(BackendError::Unsupported(_))=>false,
 };
 let mut same=backend.pixels.len()==bytes.len();
 for index in 0..bytes.len() {same=same && backend.pixels[index]==bytes[index];}
 outcome && backend.submit_calls==1 && backend.present_calls==u8::from(submit_ok)
  && backend.width==width && backend.height==height && same
}
pub fn check_default_rejection(width:u32,height:u32,bytes:&[u8])->bool {
 let mut backend=DefaultBackend {present_calls:0};
 matches!(submit_native_frame(&mut backend,width,height,bytes.to_vec()),Err(BackendError::Unsupported(_)))
  && backend.present_calls==0
}
/// Runtime correspondence between the exact production integer kernel and
/// Rust's IEEE-754 comparison over every possible f32 bit pattern.
pub fn check_dpi_scale_bits(bits:u32)->bool {
 let scale=f32::from_bits(bits);
 admits_native_dpi_scale_bits(bits)
  == (scale.is_finite() && (1.0f32/64.0..=64.0f32).contains(&scale))
}
/// Runtime property over the exact production host-surface orchestration kernel.
pub fn check_host_surface(width:u32,height:u32,destination_len:usize,stride:usize,
    render_some:bool,pixels:&[u8],fill:u8)->bool {
 let mut destination=vec![fill;destination_len];let before=destination.clone();let mut render_calls=0u8;
 let result=orchestrate_native_host_surface(width,height,&mut destination,stride,usize::MAX,
  |_|{render_calls+=1;render_some.then(||pixels.to_vec())});
 let row_bytes=(width as usize).checked_mul(4);
 let height_usize=height as usize;
 let span_ok=row_bytes.and_then(|row|surface_span(row,height_usize,stride,destination_len)).is_some();
 let expected=row_bytes.and_then(|row|row.checked_mul(height_usize));
 let exact=span_ok && render_some && expected==Some(pixels.len());
 let mut correct=result==exact && render_calls==u8::from(span_ok);
 if exact {
  let row=row_bytes.unwrap();
  for y in 0..height_usize {
   for x in 0..row {correct=correct && destination[y*stride+x]==pixels[y*row+x];}
   if y+1<height_usize {for x in row..stride {correct=correct && destination[y*stride+x]==fill;}}
  }
 } else if !span_ok || !render_some || expected!=Some(pixels.len()) {
  correct=correct && destination==before;
 }
 correct
}
/// Runtime property over the exact production native render orchestration kernel.
pub fn check_orchestration(owner_ok:bool,version_ok:bool,open:bool,width:u32,height:u32,budget:usize,
    render_some:bool,bytes:&[u8],submit_ok:bool,present_ok:bool)->bool {
 let mut backend=RecordingBackend {submit_ok,present_ok,submit_calls:0,present_calls:0,width:0,height:0,pixels:Vec::new()};
 let mut render_calls=0u8;
 let result=orchestrate_native_frame(7,if owner_ok {7}else{8},11,if version_ok {11}else{12},
  open.then_some((width,height)),budget,|_|{render_calls+=1;render_some.then(||bytes.to_vec())},&mut backend);
 let expected=cpu_budget::cpu_frame_bytes(width,height,budget);
 let admitted=owner_ok && version_ok && open && expected.is_some();
 let exact=admitted && render_some && expected==Some(bytes.len());
 let outcome=if exact {match result {
  Ok(())=>submit_ok && present_ok,
  Err(BackendError::Invalid(_))=>!submit_ok,
  Err(BackendError::Io(_))=>submit_ok && !present_ok,
  Err(BackendError::Unsupported(_))=>false,
 }} else {matches!(result,Err(BackendError::Invalid(_)))};
 let mut same=backend.pixels.len()==if exact {bytes.len()}else{0};
 if exact {for index in 0..bytes.len(){same=same && backend.pixels[index]==bytes[index];}}
 outcome && render_calls==u8::from(admitted) && backend.submit_calls==u8::from(exact)
  && backend.present_calls==u8::from(exact && submit_ok) && same
  && (!exact || (backend.width==width && backend.height==height))
}
/// Runtime property over the real owned Vec and nonempty clipping state.
pub fn check_frame(width:u32,height:u32,bytes:&[u8],seed:u8)->bool {
 let old=vec![seed;8];
 let mut canvas=Canvas::from_rgba8(2,1,old.clone()).unwrap();
 let clip=Rect {origin:Point {x:1.0,y:2.0},size:Size {width:3.0,height:4.0}};
 canvas.clip_stack.push(clip);
 let mut backend=HeadlessBackend {canvas};
 let valid=width>0 && height>0 && width as u128 * height as u128 * 4 == bytes.len() as u128;
 let result=backend.submit_native_pixels(width,height,bytes.to_vec());
 if valid {
  result.is_ok() && backend.canvas.width==width && backend.canvas.height==height
   && backend.canvas.pixels==bytes && backend.canvas.clip_stack.is_empty()
 } else {
  result.is_err() && backend.canvas.width==2 && backend.canvas.height==1
   && backend.canvas.pixels==old && backend.canvas.clip_stack==vec![clip]
 }
}
#[cfg(test)] mod tests {
 use super::*;
 #[cfg(not(miri))]
 proptest::proptest! {
  #![proptest_config(proptest::test_runner::Config::with_cases(128))]
  #[test] fn frame_property(width in 0u32..65,height in 0u32..65,bytes in proptest::collection::vec(proptest::num::u8::ANY,0..16385),seed in proptest::num::u8::ANY) {
   proptest::prop_assert!(check_frame(width,height,&bytes,seed));
   if width>0 && height>0 {proptest::prop_assert!(check_frame(width,height,&vec![seed;width as usize*height as usize*4],seed));}
  }
  #[test] fn submission_property(width in proptest::num::u32::ANY,height in proptest::num::u32::ANY,
      bytes in proptest::collection::vec(proptest::num::u8::ANY,0..257),submit_ok in proptest::bool::ANY,present_ok in proptest::bool::ANY) {
   proptest::prop_assert!(check_submission(width,height,&bytes,submit_ok,present_ok));
   proptest::prop_assert!(check_default_rejection(width,height,&bytes));
  }
  #[test] fn orchestration_property(owner_ok in proptest::bool::ANY,version_ok in proptest::bool::ANY,
      open in proptest::bool::ANY,width in 0u32..17,height in 0u32..17,budget in 0usize..2049,
      render_some in proptest::bool::ANY,bytes in proptest::collection::vec(proptest::num::u8::ANY,0..1025),
      submit_ok in proptest::bool::ANY,present_ok in proptest::bool::ANY) {
   proptest::prop_assert!(check_orchestration(owner_ok,version_ok,open,width,height,budget,render_some,&bytes,submit_ok,present_ok));
  }
  #[test] fn dpi_scale_bits_property(bits in proptest::num::u32::ANY) {
   proptest::prop_assert!(check_dpi_scale_bits(bits));
  }
  #[test] fn host_surface_property(width in 0u32..9,height in 0u32..9,
      destination_len in 0usize..513,stride in 0usize..65,render_some in proptest::bool::ANY,
      pixels in proptest::collection::vec(proptest::num::u8::ANY,0..257),fill in proptest::num::u8::ANY) {
   proptest::prop_assert!(check_host_surface(width,height,destination_len,stride,render_some,&pixels,fill));
  }
 }
 #[test] fn miri_native_frame_import() {
  for (width,height) in [(0,0),(0,1),(1,0),(1,1),(2,1),(1,2),(3,7),(u32::MAX,1),(1,u32::MAX)] {
   for len in [0,1,3,4,8,84,85] {assert!(check_frame(width,height,&vec![37;len],91));}
  }
 }
 #[test] fn miri_native_submission() {
  for submit_ok in [false,true] {for present_ok in [false,true] {
   for bytes in [&[][..],&[0][..],&[1,2,3,4][..]] {assert!(check_submission(7,9,bytes,submit_ok,present_ok));}
  }}
  assert!(check_default_rejection(7,9,&[1,2,3,4]));
 }
 #[test] fn miri_native_orchestration() {
  for owner_ok in [false,true] {for version_ok in [false,true] {for open in [false,true] {
   for (width,height,budget) in [(0,0,0),(1,1,3),(1,1,4),(2,1,8),(3,2,24)] {
    for render_some in [false,true] {for submit_ok in [false,true] {for present_ok in [false,true] {
     for bytes in [&[][..],&[1,2,3,4][..],&[7;8][..],&[9;24][..]] {
      assert!(check_orchestration(owner_ok,version_ok,open,width,height,budget,render_some,bytes,submit_ok,present_ok));
     }
    }}}
   }
  }}}
 }
 #[test] fn miri_native_dpi_scale_bits() {
  for bits in [0,1,0x3c7f_ffff,0x3c80_0000,0x3c80_0001,0x3f80_0000,
      0x427f_ffff,0x4280_0000,0x4280_0001,0x7f80_0000,0x7fc0_0000,
      0x8000_0000,0xbc80_0000,0xff80_0000,0xffc0_0000] {
   assert!(check_dpi_scale_bits(bits));
  }
 }
 #[test] fn miri_native_host_surface() {
  for (width,height,len,stride) in [(0,0,0,0),(1,1,4,4),(1,2,12,8),(2,2,24,12),(2,2,23,12)] {
   for render_some in [false,true] {for pixels in [&[][..],&[1,2,3,4][..],&[7;8][..],&[9;16][..]] {
    assert!(check_host_surface(width,height,len,stride,render_some,pixels,0xa5));
   }}
  }
 }
}
#[cfg(kani)] mod proofs {
 use super::*;
 #[kani::proof]
 #[kani::unwind(25)]
 fn native_backend_frame_import() {
  let width:u32=kani::any();let height:u32=kani::any();kani::assume(width<=8 && height<=8);
  let len:usize=kani::any();kani::assume(len<=16);
  let bytes:[u8;16]=kani::any();let seed:u8=kani::any();
  assert!(check_frame(width,height,&bytes[..len],seed));
 }
 #[kani::proof]
 #[kani::unwind(25)]
 fn native_backend_submission() {
  let width:u32=kani::any();let height:u32=kani::any();
  let len:usize=kani::any();kani::assume(len<=16);
  let bytes:[u8;16]=kani::any();let submit_ok:bool=kani::any();let present_ok:bool=kani::any();
  assert!(check_submission(width,height,&bytes[..len],submit_ok,present_ok));
  assert!(check_default_rejection(width,height,&bytes[..len]));
 }
 #[kani::proof]
 #[kani::unwind(70)]
 fn native_backend_orchestration() {
  let owner_ok:bool=kani::any();let version_ok:bool=kani::any();let open:bool=kani::any();
  let width:u32=kani::any();let height:u32=kani::any();kani::assume(width<=4 && height<=4);
  let budget:usize=kani::any();kani::assume(budget<=64);
  let len:usize=kani::any();kani::assume(len<=64);let bytes:[u8;64]=kani::any();
  let render_some:bool=kani::any();let submit_ok:bool=kani::any();let present_ok:bool=kani::any();
  assert!(check_orchestration(owner_ok,version_ok,open,width,height,budget,render_some,&bytes[..len],submit_ok,present_ok));
 }
 #[kani::proof]
 fn native_dpi_scale_bits() {
  let bits:u32=kani::any();
  assert!(check_dpi_scale_bits(bits));
 }
 #[kani::proof]
 #[kani::unwind(16)]
 fn native_host_surface() {
  let pixels:[u8;8]=kani::any();let render_some:bool=kani::any();let fill:u8=kani::any();
  assert!(check_host_surface(1,2,12,8,render_some,&pixels,fill));
  assert!(check_host_surface(1,2,12,8,true,&pixels[..7],fill));
  assert!(check_host_surface(1,2,11,8,true,&pixels,fill));
 }
}
