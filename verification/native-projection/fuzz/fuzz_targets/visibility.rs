#![no_main]
libfuzzer_sys::fuzz_target!(|data: [u8;33]| {jasper_ui_native_projection_verification::scene_close::visibility_fuzz(&data);});
