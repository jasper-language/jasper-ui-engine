#![no_main]
libfuzzer_sys::fuzz_target!(|data:[u8;80]| {jasper_ui_native_projection_verification::host_context_fuzz(&data);});
