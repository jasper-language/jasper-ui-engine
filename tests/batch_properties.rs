//! Property checks against the production decoder; these are not formal proofs.
//! 针对生产解码器的属性测试；它们不能替代形式化证明。
use jasper_ui_engine::{BatchError, PaintOp, UiBatch};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Arbitrary hostile input must return normally; successful wire forms are canonical.
    // 任意恶意字节必须正常返回；成功帧重新编码后必须保持原字节。
    #[test]
    fn arbitrary_bytes_are_total(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        if let Ok(batch) = UiBatch::decode(&bytes) {
            prop_assert_eq!(batch.encode(), bytes);
        }
    }

    // Tag-only batches exercise valid operation loops without floating-point equality.
    // 无载荷操作覆盖合法解析循环，避免浮点 NaN 的非自反比较。
    #[test]
    fn tag_only_batches_roundtrip(count in 0usize..256) {
        let batch = UiBatch::new(vec![PaintOp::PopClip; count]);
        let bytes = batch.encode();
        prop_assert_eq!(UiBatch::decode(&bytes), Ok(batch));
        for end in 0..bytes.len() {
            prop_assert!(UiBatch::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        prop_assert_eq!(UiBatch::decode(&trailing), Err(BatchError::TrailingBytes));
    }

    // A full header with insufficient tag bytes must fail before allocating ops.
    // 完整帧头缺少标签字节时，应在分配操作数组前拒绝。
    #[test]
    fn hostile_counts_reject(count in 1u32..=u32::MAX) {
        let mut bytes = b"JUEB".to_vec();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        let expected = if count > 1_000_000 { BatchError::TooLarge } else { BatchError::Truncated };
        prop_assert_eq!(UiBatch::decode(&bytes), Err(expected));
    }
}
