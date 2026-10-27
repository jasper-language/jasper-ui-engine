#![no_main]
libfuzzer_sys::fuzz_target!(|data:[u8;82]| {jasper_ui_native_projection_verification::capture_policy::fuzz(&data);});
