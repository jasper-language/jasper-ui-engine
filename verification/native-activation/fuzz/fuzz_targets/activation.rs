#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data:[u8;17]| {
    let current=u64::from_le_bytes(data[..8].try_into().unwrap());
    let expected=u64::from_le_bytes(data[8..16].try_into().unwrap());
    let enabled=data[16]&1!=0;
    jasper_ui_native_activation_verification::check_activation(current,expected,enabled);
    jasper_ui_native_activation_verification::check_activation(current,current,enabled);
});
