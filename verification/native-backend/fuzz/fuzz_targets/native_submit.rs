#![no_main]
libfuzzer_sys::fuzz_target!(|data:&[u8]| {
 if data.len()<10 {return;}
 let width=u32::from_le_bytes(data[..4].try_into().unwrap());
 let height=u32::from_le_bytes(data[4..8].try_into().unwrap());
 let submit_ok=data[8]&1!=0;let present_ok=data[9]&1!=0;
 assert!(jasper_ui_native_backend_verification::check_submission(width,height,&data[10..],submit_ok,present_ok));
 assert!(jasper_ui_native_backend_verification::check_default_rejection(width,height,&data[10..]));
});
