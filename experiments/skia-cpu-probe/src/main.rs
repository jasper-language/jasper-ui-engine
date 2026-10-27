//! Upstream Skia CPU smoke probe, not a delivered Jasper renderer or a proof.
//! 上游 Skia CPU 冒烟实验，不是已交付的 Jasper 后端或数学证明。
use skia_safe::{surfaces, AlphaType, ClipOp, Color, ColorType, ImageInfo, Paint, Rect};
use std::{fs, path::PathBuf};

fn main() {
    let destination = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    fs::create_dir_all(&destination).unwrap();
    let mut surface = surfaces::raster_n32_premul((64, 48)).expect("CPU surface");
    let canvas = surface.canvas();
    canvas.clear(Color::WHITE);
    let mut paint = Paint::default();
    paint.set_anti_alias(false);
    paint.set_color(Color::BLUE);
    canvas.draw_rect(Rect::from_xywh(4.0, 4.0, 16.0, 16.0), &paint);
    canvas.save();
    canvas.clip_rect(
        Rect::from_xywh(32.0, 4.0, 16.0, 16.0),
        ClipOp::Intersect,
        false,
    );
    paint.set_color(Color::RED);
    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 64.0, 48.0), &paint);
    // A disjoint nested clip must draw nothing rather than disable clipping.
    // 嵌套无交集裁剪必须不绘制，不能退化为取消裁剪。
    canvas.clip_rect(
        Rect::from_xywh(0.0, 32.0, 8.0, 8.0),
        ClipOp::Intersect,
        false,
    );
    paint.set_color(Color::BLACK);
    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 64.0, 48.0), &paint);
    canvas.restore();
    paint.set_color(Color::from_argb(128, 255, 0, 0));
    canvas.draw_rect(Rect::from_xywh(4.0, 28.0, 16.0, 16.0), &paint);
    let info = ImageInfo::new((64, 48), ColorType::RGBA8888, AlphaType::Unpremul, None);
    let mut pixels = vec![0u8; 64 * 48 * 4];
    assert!(surface.read_pixels(&info, &mut pixels, 64 * 4, (0, 0)));
    let pixel = |x: usize, y: usize| &pixels[(y * 64 + x) * 4..(y * 64 + x + 1) * 4];
    assert_eq!(pixel(8, 8), [0, 0, 255, 255]);
    assert_eq!(pixel(36, 8), [255, 0, 0, 255]);
    assert_eq!(pixel(24, 8), [255, 255, 255, 255]);
    assert_eq!(pixel(36, 36), [255, 255, 255, 255]);
    assert_eq!(pixel(8, 32), [255, 127, 127, 255]);
    fs::write(destination.join("frame.rgba"), &pixels).unwrap();
    let mut ppm = b"P6\n64 48\n255\n".to_vec();
    for pixel in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    fs::write(destination.join("frame.ppm"), ppm).unwrap();
    println!("Skia CPU probe passed: fill, nested/disjoint clip, alpha, RGBA readback; no window/GPU/font calls");
}
