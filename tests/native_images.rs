//! P0-A host-injected image resources on the native CPU document path: decode by content,
//! object-fit pixels, replacement invalidation, decode/cache errors and resource removal.
//! P0-A 原生 CPU 文档路径上的宿主注入图片资源：按内容解码、object-fit 像素、替换失效、
//! 解码/缓存错误与资源删除。
#![cfg(feature = "native-engine-cpu")]

use jasper_ui_engine::native::style::{ImageStyle, ObjectFit};
use jasper_ui_engine::native::{ImageError, ImageNodeError};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, Length, Style};
use jasper_ui_engine::Engine;
#[cfg(feature = "gpu-pending")]
use jasper_ui_engine::native::host::GpuRenderer;

const OWNER: u64 = 77;
const IMAGE: NodeId = NodeId(2);
const W: u32 = 40;
const H: u32 = 40;

fn png(pixels: &[[u8; 4]], width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba(pixels[(y * width + x) as usize])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .background(Color::WHITE),
            )
            .child(
                Node::container(IMAGE).with_style(
                    Style::default()
                        .size(Length::Px(20.0), Length::Px(20.0))
                        .image(Some(ImageStyle::new("logo").fit(ObjectFit::Fill))),
                ),
            ),
    )
}

fn render(engine: &mut Engine, version: u64) -> Vec<u8> {
    let mut pixels = vec![0u8; (W * H * 4) as usize];
    assert!(engine.render_to_surface(
        OWNER,
        version,
        &mut pixels,
        (W * 4) as usize,
        1 << 24
    ));
    pixels
}

fn rgba(frame: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * W as usize + x) * 4;
    frame[i..i + 4].try_into().unwrap()
}

#[test]
fn host_bytes_decode_draw_replace_and_remove() {
    let source = scene();
    let mut engine = Engine::new(OWNER, &source, &[], W, H).unwrap();
    let initial = png(
        &[
            [255, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 255, 255],
        ],
        2,
        2,
    );
    let info = engine
        .insert_image(OWNER, "logo", &initial, 1.0)
        .unwrap();
    assert_eq!((info.pixel_width, info.pixel_height), (2, 2));
    let first = render(&mut engine, source.version());
    assert_eq!(rgba(&first, 4, 4), [255, 0, 0, 255]);
    assert_eq!(rgba(&first, 15, 15), [0, 0, 255, 255]);

    let blue = png(&[[0, 255, 0, 255]], 1, 1);
    engine.insert_image(OWNER, "logo", &blue, 1.0).unwrap();
    let replaced = render(&mut engine, source.version());
    assert_eq!(rgba(&replaced, 4, 4), [0, 255, 0, 255]);
    assert_ne!(replaced, first);

    assert_eq!(engine.remove_image(OWNER, "logo"), Some(true));
    let removed = render(&mut engine, source.version());
    assert_eq!(rgba(&removed, 4, 4), [255, 255, 255, 255]);
    assert_eq!(
        engine.image_report(OWNER).unwrap().failed[0].error,
        ImageNodeError::Missing
    );
}

#[test]
fn malformed_bytes_are_rejected_without_replacing_the_live_resource() {
    let source = scene();
    let mut engine = Engine::new(OWNER, &source, &[], W, H).unwrap();
    let red = png(&[[255, 0, 0, 255]], 1, 1);
    engine.insert_image(OWNER, "logo", &red, 1.0).unwrap();
    assert!(matches!(
        engine.insert_image(OWNER, "logo", b"GIF89a", 1.0),
        Err(ImageError::UnsupportedFormat)
    ));
    let frame = render(&mut engine, source.version());
    assert_eq!(rgba(&frame, 4, 4), [255, 0, 0, 255]);
    assert!(engine.image_report(OWNER).unwrap().failed.is_empty());
    assert_eq!(engine.insert_image(OWNER + 1, "x", &red, 1.0), Err(ImageError::Rejected));
}

#[test]
fn svg_document_is_rasterized_at_the_node_content_size() {
    let source = scene();
    let mut engine = Engine::new(OWNER, &source, &[], W, H).unwrap();
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
        <rect width="20" height="20" fill="#ff8800"/>
    </svg>"##;
    engine.insert_image(OWNER, "logo", svg, 1.0).unwrap();
    let frame = render(&mut engine, source.version());
    assert!(frame.chunks_exact(4).any(|p| p[0] > 240 && p[1] > 120 && p[2] < 20));
}

#[cfg(feature = "gpu-pending")]
#[test]
fn image_frame_is_consistent_between_cpu_and_gpu_paths() {
    let source = scene();
    let mut engine = Engine::new(OWNER, &source, &[], W, H).unwrap();
    let image = png(
        &[
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
        ],
        2,
        2,
    );
    engine.insert_image(OWNER, "logo", &image, 1.0).unwrap();
    let cpu = render(&mut engine, source.version());
    engine.invalidate_frame(OWNER);
    let mut gpu = GpuRenderer::new_headless().expect("wgpu adapter required");
    let gpu_frame = engine.render_gpu(OWNER, source.version(), &mut gpu, 1 << 24).unwrap();
    for (x, y) in [(4, 4), (15, 4), (4, 15), (15, 15)] {
        let a = rgba(&cpu, x, y);
        let i = (y * W as usize + x) * 4;
        let b: [u8; 4] = gpu_frame[i..i + 4].try_into().unwrap();
        assert!(
            a.into_iter().zip(b).all(|(left, right)| left.abs_diff(right) <= 1),
            "({x},{y}): {a:?} vs {b:?}"
        );
    }
}
