#![no_main]
libfuzzer_sys::fuzz_target!(|data:&[u8]| {
 if data.len()<8 {return;}
 let width=u32::from(data[0]&7);let height=u32::from(data[1]&7);
 let destination_len=u16::from_le_bytes([data[2],data[3]]) as usize;
 let stride=u16::from_le_bytes([data[4],data[5]]) as usize;
 assert!(jasper_ui_native_backend_verification::check_host_surface(
  width,height,destination_len.min(512),stride.min(64),data[6]&1!=0,&data[8..],data[7]));
});
