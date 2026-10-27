#![no_main]
libfuzzer_sys::fuzz_target!(|data:[u8;16]| {
    let width=u32::from_le_bytes(data[..4].try_into().unwrap());
    let height=u32::from_le_bytes(data[4..8].try_into().unwrap());
    let budget=u64::from_le_bytes(data[8..].try_into().unwrap()) as usize;
    jasper_ui_native_activation_verification::check_cpu_frame(width,height,budget);
    jasper_ui_native_activation_verification::check_cpu_frame(width&65535,height&65535,budget);
});
