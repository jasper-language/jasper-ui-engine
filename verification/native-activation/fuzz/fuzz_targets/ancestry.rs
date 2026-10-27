#![no_main]
libfuzzer_sys::fuzz_target!(|data: [u8;20]| {jasper_ui_native_activation_verification::ancestry::fuzz(&data);});
