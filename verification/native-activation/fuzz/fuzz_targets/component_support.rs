#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data:[u8;42]| {
    jasper_ui_native_activation_verification::check_component_support(data[0]&1!=0,u16::from_le_bytes([data[1],data[2]]));
    let word=|at:usize|u64::from_le_bytes(data[at..at+8].try_into().unwrap());
    let count=u16::from_le_bytes([data[40],data[41]]) as usize;
    jasper_ui_native_activation_verification::check_component_frame_admission(
        data[3]&1!=0,data[3]&2!=0,count,word(8),word(16),data[3]&4!=0,data[3]&8!=0);
});
