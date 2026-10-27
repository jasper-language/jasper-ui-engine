#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data:[u8;3]| {
    jasper_ui_native_activation_verification::check_component_support(data[0]&1!=0,u16::from_le_bytes([data[1],data[2]]));
});
