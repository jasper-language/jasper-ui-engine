#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    jasper_ui_view_slot_verification::check_trace(data);
});
