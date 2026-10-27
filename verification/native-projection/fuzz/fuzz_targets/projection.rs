#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: [u8; 9]| {
    let version = u64::from_le_bytes(data[..8].try_into().unwrap());
    jasper_ui_native_projection_verification::check_projection(version, data[8] & 1 != 0);
});
