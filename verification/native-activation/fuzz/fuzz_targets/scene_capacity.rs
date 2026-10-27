#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data:[u8;24]| {
    let a=u64::from_le_bytes(data[..8].try_into().unwrap()) as usize;
    let b=u64::from_le_bytes(data[8..16].try_into().unwrap()) as usize;
    let c=u64::from_le_bytes(data[16..24].try_into().unwrap()) as usize;
    jasper_ui_native_activation_verification::check_scene_capacity(a,b,c);
    jasper_ui_native_activation_verification::check_scene_capacity(a%4097,b%4097,c%4097);
});
