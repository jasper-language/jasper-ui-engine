#![no_main]
libfuzzer_sys::fuzz_target!(|data:[u8;2]| {jasper_ui_native_activation_verification::semantic_tag::check(data[0],data[1]);});
