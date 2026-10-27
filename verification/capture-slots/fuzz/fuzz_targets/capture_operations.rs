#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    jasper_ui_capture_verification::check_operations(data);
    if data.len() >= 40 {
        let n = |i| u64::from_le_bytes(data[i..i+8].try_into().unwrap());
        let (a,b,q,x,y) = (n(0),n(8),n(16),n(24),n(32));
        jasper_ui_capture_verification::check_two_keys(a,b,q,Some(x),Some(y));
        jasper_ui_capture_verification::check_two_keys(a,b,a,None,Some(y));
        jasper_ui_capture_verification::check_two_keys(a,b,b,Some(x),None);
        jasper_ui_capture_verification::check_two_keys(a,a,a,Some(x),Some(y));
    }
});
