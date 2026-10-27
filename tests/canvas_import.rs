//! Exercise the actual engine API as well as the extracted proof kernel.
//! 同时验证真实引擎 API，避免仅测试提取适配层。
use jasper_ui_engine::{raster::Canvas, style::Color};

#[test]
fn imported_buffer_reaches_software_rendering() {
    let buffer = vec![17; 16];
    let pointer = buffer.as_ptr();
    let mut canvas = Canvas::from_rgba8(2, 2, buffer).unwrap();
    assert_eq!((canvas.width(), canvas.height()), (2, 2));
    assert_eq!(canvas.pixels().as_ptr(), pointer);
    assert_eq!(canvas.pixels(), &[17; 16]);
    canvas.clear(Color::new(1, 2, 3, 4));
    assert_eq!(canvas.pixels(), &[1, 2, 3, 4].repeat(4));
    assert_eq!(canvas.pixels().as_ptr(), pointer);
}

#[test]
fn rejected_import_returns_original_owned_buffer() {
    for (width, height, len) in [
        (0, 1, 4),
        (1, 0, 4),
        (1, 1, 3),
        (1, 1, 5),
        (u32::MAX, u32::MAX, 16),
    ] {
        let buffer = vec![99; len];
        let pointer = buffer.as_ptr();
        let capacity = buffer.capacity();
        let returned = Canvas::from_rgba8(width, height, buffer).unwrap_err();
        assert_eq!(returned, vec![99; len]);
        assert_eq!(returned.as_ptr(), pointer);
        assert_eq!(returned.capacity(), capacity);
    }
}
