//! Small deterministic production paths for the Miri interpreter.
//! 用于 Miri 解释器的有界生产路径，覆盖文本分配与错误清理。
use jasper_ui_engine::{Color, PaintOp, Point, UiBatch};

#[test]
fn text_payload_truncation_and_allocation_lifetimes() {
    let bytes = UiBatch::new(vec![PaintOp::DrawText {
        origin: Point::new(1.0, 2.0),
        text: "Jasper 中文 🦀".to_owned(),
        font_size: 16.0,
        color: Color::RED,
    }])
    .encode();
    for end in 0..bytes.len() {
        assert!(UiBatch::decode(&bytes[..end]).is_err());
    }
    let decoded = UiBatch::decode(&bytes).unwrap();
    drop(bytes);
    // The resulting String must own its data after the wire buffer is dropped.
    // 原帧释放之后，结果中的 String 必须仍然持有有效内容。
    match &decoded.ops[0] {
        PaintOp::DrawText { text, .. } => assert_eq!(text, "Jasper 中文 🦀"),
        _ => panic!("wrong decoded operation"),
    }
}

#[test]
fn malformed_utf8_and_extreme_lengths() {
    let mut prefix = b"JUEB\x01\x00\x01\x00\x00\x00\x04".to_vec();
    prefix.extend_from_slice(&[0; 8]); // origin / 起点
    for len in [0u32, 1, 16 * 1024 * 1024, u32::MAX] {
        let mut frame = prefix.clone();
        frame.extend_from_slice(&len.to_le_bytes());
        frame.push(0xff);
        assert!(UiBatch::decode(&frame).is_err());
    }
}
