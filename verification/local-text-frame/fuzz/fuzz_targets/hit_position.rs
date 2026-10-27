#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|bytes:&[u8]|{if bytes.len()!=12{return;}let position=i32::from_le_bytes(bytes[0..4].try_into().unwrap());let len=u64::from_le_bytes(bytes[4..12].try_into().unwrap());jasper_ui_local_text_storage_verification::check_hit(position,len as usize);});
