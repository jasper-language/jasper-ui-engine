#![no_main]
libfuzzer_sys::fuzz_target!(|data: [u8;25]| {jasper_ui_native_projection_verification::scene_close::fuzz(&data);});
