#![no_main]
libfuzzer_sys::fuzz_target!(|data:&[u8]| {
 if data.len()<9 {return;}
 let width=u32::from_le_bytes(data[..4].try_into().unwrap());
 let height=u32::from_le_bytes(data[4..8].try_into().unwrap());
 assert!(jasper_ui_native_backend_verification::check_frame(width,height,&data[9..],data[8]));
});
