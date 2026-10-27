#![no_main]
use jasper_ui_engine::UiBatch;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Successful wire decoding must reproduce the original bits, including NaNs.
    // 成功解码必须保留原帧位模式，包括 NaN；不以浮点相等代替。
    if let Ok(batch) = UiBatch::decode(data) {
        assert_eq!(batch.encode(), data);
    }
});
