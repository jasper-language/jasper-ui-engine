#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data:[u8;16]| {
    let index=u64::from_le_bytes(data[..8].try_into().unwrap()) as usize;
    let children=u64::from_le_bytes(data[8..].try_into().unwrap()) as usize;
    jasper_ui_native_activation_verification::check_insert_index(index,children);
});
