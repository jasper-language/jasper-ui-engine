#![no_main]
libfuzzer_sys::fuzz_target!(|data:&[u8]| {
 if data.len()<12 {return;}
 let flags=data[0];
 let width=u32::from(data[1]&15);let height=u32::from(data[2]&15);
 let budget=u16::from_le_bytes([data[3],data[4]]) as usize;
 let payload=&data[12..];
 assert!(jasper_ui_native_backend_verification::check_orchestration(
  flags&1!=0,flags&2!=0,flags&4!=0,width,height,budget,flags&8!=0,payload,
  flags&16!=0,flags&32!=0));
});
