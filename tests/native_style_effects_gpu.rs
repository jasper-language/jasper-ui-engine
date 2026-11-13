//! E2/E3/E4/E9 on the native GPU path (Vello hybrid on wgpu): the same anyrender scene as the
//! CPU path, including the engine-painted outline and text decoration overlays.
//! 原生 GPU 路径（wgpu 上的 Vello hybrid）上的 E2/E3/E4/E9：与 CPU 路径同一 anyrender 场景，含引擎绘制的外框与文字装饰。
#![cfg(feature = "gpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::host::GpuRenderer;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{
    BoxShadow, Color, Display, FlexDirection, Length, Outline, Style, TextDecoration,
};

const OWNER: u64 = 9;
const W: u32 = 200;
const H: u32 = 120;

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT")).unwrap()
}

/// Row at (20,20): rounded blue box with spread shadow and outline (x 20..60), a 50% blue box
/// (x 80..120), underlined text (x 140..).
fn scene() -> Scene {
    let base = |w: f32| Style::default().size(Length::Px(w), Length::Px(40.0));
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .padding(Edges::all(20.0))
                    .gap(20.0)
                    .display(Display::Flex)
                    .direction(FlexDirection::Row)
                    .background(Color::WHITE),
            )
            .child(
                Node::container(NodeId(2)).with_style(
                    base(40.0)
                        .background(Color::rgb(0, 0, 255))
                        .corner_radius(12.0)
                        .shadow(BoxShadow::new(0.0, 12.0, 0.0, 0.0, Color::BLACK))
                        .outline(Some(Outline {
                            width: 2.0,
                            offset: 3.0,
                            color: Color::rgb(255, 0, 0),
                        })),
                ),
            )
            .child(
                Node::container(NodeId(3))
                    .with_style(base(40.0).background(Color::rgb(0, 0, 255)).opacity(0.5)),
            )
            .child(
                Node::text(NodeId(4), "xxxx").with_style(
                    Style::default()
                        .text(20.0, Color::BLACK)
                        .text_decoration(TextDecoration::UNDERLINE),
                ),
            ),
    )
}

fn px(frame: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    frame[i..i + 4].try_into().unwrap()
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= 3)
}

#[test]
fn gpu_paints_extended_styles_like_cpu() {
    let source = scene();
    let v = source.version();
    let mut gpu = GpuRenderer::new_headless().expect("a wgpu adapter is required for GPU acceptance");
    let mut engine = Engine::with_fonts(OWNER, &source, &[&font()], W, H).unwrap();
    let frame = engine.render_gpu(OWNER, v, &mut gpu, 1 << 24).expect("gpu frame");
    let mut cpu_engine = Engine::with_fonts(OWNER, &source, &[&font()], W, H).unwrap();
    let mut cpu = vec![0u8; (W * H * 4) as usize];
    assert!(cpu_engine.render_to_surface(OWNER, v, &mut cpu, (W * 4) as usize, 1 << 24));

    let white = [255, 255, 255, 255];
    // (22,22): outside the 12px box corner arc (centre 32,32), inside the ring's 15px hole.
    assert!(near(px(&frame, 22, 22), white), "E2 corner tip");
    assert!(near(px(&frame, 40, 40), [0, 0, 255, 255]), "E2 body");
    assert!(near(px(&frame, 40, 70), [0, 0, 0, 255]), "E3 offset shadow below the box");
    assert!(near(px(&frame, 16, 40), [255, 0, 0, 255]), "E9 outline ring");
    assert!(near(px(&frame, 18, 40), white), "E9 outline gap");
    assert!(near(px(&frame, 100, 40), [127, 127, 255, 255]), "E4 50% blue");
    // Same sample points as the CPU frame.
    for (x, y) in [(22, 22), (20, 20), (40, 40), (40, 70), (16, 40), (18, 40), (100, 40)] {
        assert!(near(px(&frame, x, y), px(&cpu, x, y)), "({x},{y}) GPU vs CPU");
    }
    // E9 underline: some row under the text is dark across the text span on both paths.
    let dark_row = |f: &[u8]| {
        (0..H).any(|y| (144..170).all(|x| px(f, x, y)[0] < 128))
    };
    assert!(dark_row(&cpu), "CPU underline");
    assert!(dark_row(&frame), "GPU underline");
}

/// E7/E8 on the GPU path: a translated, outlined box and a mid-transition frame match the CPU.
/// GPU 路径上的 E7/E8：平移且带外框的盒子与过渡中间帧与 CPU 一致。
#[test]
fn gpu_paints_transforms_and_transition_frames_like_cpu() {
    use jasper_ui_engine::scene::ScenePatch;
    use jasper_ui_engine::style::{AnimatedProperties, Easing, Transform2D, Transition};
    let rule = Transition::new(AnimatedProperties::OPACITY, 100.0, Easing::LINEAR);
    let boxed = |opacity: f32| {
        Style::default()
            .size(Length::Px(40.0), Length::Px(40.0))
            .background(Color::rgb(0, 0, 255))
            .outline(Some(Outline {
                width: 2.0,
                offset: 2.0,
                color: Color::rgb(255, 0, 0),
            }))
            .transform(Transform2D::translate(60.0, 0.0))
            .opacity(opacity)
            .transition(rule)
    };
    let source = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .padding(Edges::all(20.0))
                    .background(Color::WHITE),
            )
            .child(Node::container(NodeId(2)).with_style(boxed(1.0))),
    );
    let mut gpu = GpuRenderer::new_headless().expect("a wgpu adapter is required for GPU acceptance");
    let mut gpu_engine = Engine::with_fonts(OWNER, &source, &[&font()], W, H).unwrap();
    let mut cpu_engine = Engine::with_fonts(OWNER, &source, &[&font()], W, H).unwrap();
    let mut v = source.version();
    for engine in [&mut gpu_engine, &mut cpu_engine] {
        engine.animate(OWNER, 0.0).unwrap();
        engine
            .try_apply(
                OWNER,
                v,
                v + 1,
                &[ScenePatch::UpdateStyle {
                    id: NodeId(2),
                    style: boxed(0.0),
                }],
            )
            .unwrap();
        engine.animate(OWNER, 0.0).unwrap();
        engine.animate(OWNER, 50.0).unwrap();
    }
    v += 1;
    let frame = gpu_engine.render_gpu(OWNER, v, &mut gpu, 1 << 24).expect("gpu frame");
    let mut cpu = vec![0u8; (W * H * 4) as usize];
    assert!(cpu_engine.render_to_surface(OWNER, v, &mut cpu, (W * 4) as usize, 1 << 24));
    let white = [255, 255, 255, 255];
    assert!(near(px(&frame, 100, 40), [127, 127, 255, 255]), "half-faded moved box");
    assert!(near(px(&frame, 77, 40), [255, 127, 127, 255]), "half-faded moved outline");
    assert!(near(px(&frame, 40, 40), white), "layout place is empty");
    for (x, y) in [(100, 40), (77, 40), (40, 40), (17, 40), (123, 40)] {
        assert!(near(px(&frame, x, y), px(&cpu, x, y)), "({x},{y}) GPU vs CPU");
    }
}
