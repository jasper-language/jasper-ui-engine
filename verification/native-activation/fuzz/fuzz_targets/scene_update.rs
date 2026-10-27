#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data:[u8;42]| {
    let word=|i:usize|u64::from_le_bytes(data[i..i+8].try_into().unwrap());
    let count=u16::from_le_bytes([data[40],data[41]]) as usize;let open=data[0]&1!=0;
    jasper_ui_native_activation_verification::check_scene_update(word(0),word(8),word(16),word(24),word(32),count,open);
    jasper_ui_native_activation_verification::check_scene_update(word(0),word(0),word(16),word(16),word(16).wrapping_add(1),count%4097,open);
});
