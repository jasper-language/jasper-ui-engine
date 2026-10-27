#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|bytes:&[u8]|{if bytes.len()!=16{return;}let w=u32::from_le_bytes(bytes[0..4].try_into().unwrap());let h=u32::from_le_bytes(bytes[4..8].try_into().unwrap());let b=u64::from_le_bytes(bytes[8..16].try_into().unwrap());jasper_ui_local_text_storage_verification::check(w,h,b as usize);});
