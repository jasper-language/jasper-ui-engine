#![no_main]
libfuzzer_sys::fuzz_target!(|bits:u32| {
 assert!(jasper_ui_native_backend_verification::check_dpi_scale_bits(bits));
});
