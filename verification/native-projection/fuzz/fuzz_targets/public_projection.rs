#![no_main]
libfuzzer_sys::fuzz_target!(|data: [u8;89]| {jasper_ui_native_projection_verification::public_fuzz(&data);});
