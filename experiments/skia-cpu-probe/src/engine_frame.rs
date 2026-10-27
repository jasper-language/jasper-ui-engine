//! Experimental real-engine scene/batch replay; not an accepted production backend.
//! 真实引擎场景/批指令重放实验，尚未通过生产接入验收。
use jasper_ui_engine::{
    canvas::Canvas as RecordedCanvas,
    geometry::Edges,
    layout::LayoutEngine,
    paint::{paint, PaintOp},
    resource::UiBytesSlot,
    style::Overflow,
    Color, Length, Node, NodeId, Point, Rect, Scene, SimpleTextEngine, Size, Style,
    TaffyLayoutEngine, UiBatch,
};
use skia_safe::{surfaces, AlphaType, ClipOp, ColorType, ImageInfo, Paint, PaintStyle};
use skia_safe::{Data, Font, FontMgr};
use std::{fs, path::PathBuf};

fn color(c: Color) -> skia_safe::Color {
    skia_safe::Color::from_argb(c.a, c.r, c.g, c.b)
}
fn rect(r: Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(r.x(), r.y(), r.width(), r.height())
}

// Fixed-frame prototype: reject text without injected fonts and malformed clip streams
// before allocating or replaying; no external framebuffer can be partially updated.
// 固定帧原型：分配/重放前拒绝未注入字体的文字及非法裁剪流，不部分写入外部帧缓冲。
fn replay(batch: &UiBatch, font: Option<&Font>) -> Result<Vec<u8>, &'static str> {
    let mut depth = 0usize;
    for (index, op) in batch.ops.iter().enumerate() {
        match op {
            PaintOp::DrawText { .. } if font.is_none() => {
                return Err("font-injected text not integrated")
            }
            PaintOp::Clear(_) if index != 0 => {
                return Err("experiment requires clear at frame start")
            }
            PaintOp::PushClip(_) => depth += 1,
            PaintOp::PopClip if depth == 0 => return Err("clip stack underflow"),
            PaintOp::PopClip => depth -= 1,
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unbalanced clip stack");
    }
    let mut surface = surfaces::raster_n32_premul((320, 200)).ok_or("surface allocation")?;
    let canvas = surface.canvas();
    for op in &batch.ops {
        let mut brush = Paint::default();
        brush.set_anti_alias(false);
        match op {
            PaintOp::Clear(c) => {
                canvas.clear(color(*c));
            }
            PaintOp::FillRect { rect: r, color: c } => {
                brush.set_color(color(*c));
                canvas.draw_rect(rect(*r), &brush);
            }
            PaintOp::StrokeRect {
                rect: r,
                color: c,
                width,
            } => {
                if *width > 0.0 {
                    brush.set_color(color(*c));
                    brush.set_style(PaintStyle::Stroke);
                    brush.set_stroke_width(*width);
                    canvas.draw_rect(rect(*r), &brush);
                }
            }
            PaintOp::Line {
                from,
                to,
                color: c,
                width,
            } => {
                if *width > 0.0 {
                    brush.set_color(color(*c));
                    brush.set_stroke_width(*width);
                    canvas.draw_line((from.x, from.y), (to.x, to.y), &brush);
                }
            }
            PaintOp::PushClip(r) => {
                canvas.save();
                canvas.clip_rect(rect(*r), ClipOp::Intersect, false);
            }
            PaintOp::PopClip => {
                canvas.restore();
            }
            PaintOp::DrawText {
                origin,
                text,
                font_size,
                color: c,
            } => {
                let mut font = font.expect("preflight requires injected font").clone();
                font.set_size(*font_size);
                brush.set_color(color(*c));
                let baseline = origin.y - font.metrics().1.ascent;
                canvas.draw_str(text, (origin.x, baseline), &font, &brush);
            }
        }
    }
    let info = ImageInfo::new((320, 200), ColorType::RGBA8888, AlphaType::Unpremul, None);
    let mut rgba = vec![0u8; 320 * 200 * 4];
    if !surface.read_pixels(&info, &mut rgba, 320 * 4, (0, 0)) {
        return Err("RGBA readback");
    }
    Ok(rgba)
}

fn fixture(with_text: bool) -> Scene {
    let mut commands = RecordedCanvas::new();
    commands.fill_rect(Rect::new(0.0, 0.0, 48.0, 32.0), Color::RED);
    commands.stroke_rect(Rect::new(60.0, 4.0, 40.0, 24.0), Color::WHITE, 2.0);
    commands.line(
        Point::new(110.0, 4.0),
        Point::new(150.0, 28.0),
        Color::BLUE,
        2.0,
    );
    // This off-canvas fill must be clipped by the actual paint pass.
    // 该越界填充必须被真实绘制阶段产生的内容裁剪限制。
    commands.fill_rect(Rect::new(200.0, 0.0, 100.0, 100.0), Color::RED);
    let mut header = Node::rect(NodeId(2)).with_style(
        Style::default()
            .size(Length::Px(296.0), Length::Px(36.0))
            .padding(Edges::all(8.0))
            .background(Color::new(40, 100, 180, 255)),
    );
    if with_text {
        header = header.child(
            Node::text(NodeId(4), "Jasper UI")
                .with_style(Style::default().text(18.0, Color::WHITE)),
        );
    }
    Scene::try_new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(320.0), Length::Px(200.0))
                    .padding(Edges::all(12.0))
                    .gap(12.0)
                    .background(Color::new(24, 28, 36, 255)),
            )
            .child(header)
            .child(
                Node::canvas(NodeId(3), commands).with_style(
                    Style::default()
                        .size(Length::Px(180.0), Length::Px(80.0))
                        .background(Color::new(48, 56, 68, 255))
                        .overflow(Overflow::Hidden),
                ),
            ),
    )
    .unwrap()
}

fn main() {
    let destination = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    fs::create_dir_all(&destination).unwrap();
    // The host explicitly injects bytes. No system-font manager or family lookup.
    // 宿主显式注入字体字节，不使用系统字体管理器或字体族查找。
    let mut font_resource = std::env::args_os().nth(2).map(|path| {
        let bytes = fs::read(path).expect("host-provided font bytes");
        let mut slot = UiBytesSlot::new(7, 99);
        let key = slot.install(7, bytes).unwrap();
        assert!(slot.bytes(8, key).is_none());
        let manager = FontMgr::custom_empty().expect("custom memory font manager for this target");
        let face = manager
            .new_from_data(Data::new_copy(slot.bytes(7, key).unwrap()), None)
            .expect("injected font parses");
        let font = Font::from_typeface(face, 18.0);
        assert_ne!(font.unichar_to_glyph('J' as i32), 0);
        (font, slot, key)
    });
    let font = font_resource.as_ref().map(|bundle| &bundle.0);
    let scene = fixture(font.is_some());
    let layout = TaffyLayoutEngine
        .layout(&scene, Size::new(320.0, 200.0), &SimpleTextEngine)
        .unwrap();
    let display = paint(&layout, Color::WHITE);
    let batch = UiBatch::new(display.ops);
    let encoded = batch.encode();
    let decoded = UiBatch::decode(&encoded).unwrap();
    assert_eq!(batch, decoded);
    let rgba = replay(&decoded, font).unwrap();
    if font.is_some() {
        let baseline_batch = UiBatch::new(
            decoded
                .ops
                .iter()
                .filter(|op| !matches!(op, PaintOp::DrawText { .. }))
                .cloned()
                .collect(),
        );
        let baseline = replay(&baseline_batch, None).unwrap();
        let changed = rgba
            .chunks_exact(4)
            .zip(baseline.chunks_exact(4))
            .enumerate()
            .filter(|(_, (actual, before))| actual != before)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert!(!changed.is_empty());
        assert!(changed
            .iter()
            .all(|index| (12..308).contains(&(index % 320)) && (12..48).contains(&(index / 320))));
        println!(
            "Injected-font experiment: {} glyph pixels changed inside header",
            changed.len()
        );
    }
    let pixel = |x: usize, y: usize| &rgba[(y * 320 + x) * 4..(y * 320 + x + 1) * 4];
    assert_eq!(pixel(0, 0), [24, 28, 36, 255]);
    assert_eq!(pixel(300, 20), [40, 100, 180, 255]);
    assert_eq!(pixel(20, 68), [220, 50, 47, 255]);
    assert_eq!(pixel(220, 68), [24, 28, 36, 255]);
    assert!(matches!(
        replay(&UiBatch::new(vec![PaintOp::PopClip]), None),
        Err("clip stack underflow")
    ));
    assert!(matches!(
        replay(
            &UiBatch::new(vec![PaintOp::PushClip(Rect::new(0.0, 0.0, 1.0, 1.0))]),
            None
        ),
        Err("unbalanced clip stack")
    ));
    assert!(matches!(
        replay(
            &UiBatch::new(vec![PaintOp::DrawText {
                origin: Point::ZERO,
                text: "text".into(),
                font_size: 12.0,
                color: Color::WHITE
            }]),
            None
        ),
        Err("font-injected text not integrated")
    ));
    let prefix = if font.is_some() {
        "engine-font-frame"
    } else {
        "engine-frame"
    };
    fs::write(destination.join(format!("{prefix}.rgba")), &rgba).unwrap();
    fs::write(destination.join(format!("{prefix}.batch")), &encoded).unwrap();
    let mut ppm = b"P6\n320 200\n255\n".to_vec();
    for p in rgba.chunks_exact(4) {
        ppm.extend_from_slice(&p[..3]);
    }
    fs::write(destination.join(format!("{prefix}.ppm")), ppm).unwrap();
    if let Some((font, mut slot, key)) = font_resource.take() {
        drop(font);
        assert!(slot.release(7, key).is_some());
        assert!(slot.bytes(7, key).is_none());
    }
    println!("Engine/Skia experiment passed: {} layout boxes, {} real paint ops, {} encoded bytes; 4 pixel checks and explicit clip/text rejection", layout.boxes.len(), decoded.ops.len(), encoded.len());
}
