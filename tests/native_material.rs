//! E6 gradients and E5 materials (frosted glass) on the native CPU path: headless pixel
//! assertions for gradient direction, stop positions, radial centre, corner clipping, opacity,
//! transform and transitions; backdrop blur smoothness, saturation and tint values, rounded
//! clipping, stroke, budget degradation with report, and whole-frame rejection.
//! 原生 CPU 路径上的 E6 渐变与 E5 材质（毛玻璃）：渐变方向、色标位置、径向中心、圆角裁剪、透明度、变换与过渡的
//! 无头像素断言；背景模糊平滑度、饱和度与着色取值、圆角裁剪、描边、预算降级与报告、整帧拒绝。
#![cfg(feature = "native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::{BackdropBudget, BackdropDegradation, SceneUpdateError};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::style::{
    AnimatedProperties, Backdrop, Color, Display, Easing, FlexDirection, Gradient, GradientStop,
    Length, Style, Transform2D, Transition,
};

const OWNER: u64 = 13;
const W: u32 = 160;
const H: u32 = 120;
const BG: [u8; 4] = [255, 255, 255, 255];

fn font() -> Vec<u8> {
    std::fs::read(std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT")).unwrap()
}

/// White W×H root with 20px padding; children flow in a column from (20, 20).
fn root() -> Node {
    Node::container(NodeId(1)).with_style(
        Style::default()
            .size(Length::Px(W as f32), Length::Px(H as f32))
            .padding(Edges::all(20.0))
            .gap(10.0)
            .display(Display::Flex)
            .direction(FlexDirection::Column)
            .background(Color::WHITE),
    )
}

fn engine(scene: &Scene) -> Engine {
    Engine::with_fonts(OWNER, scene, &[&font()], W, H).expect("native import")
}

fn frame(engine: &mut Engine, version: u64) -> Vec<u8> {
    let mut bytes = vec![0u8; (W * H * 4) as usize];
    assert!(engine.render_to_surface(OWNER, version, &mut bytes, (W * 4) as usize, 1 << 24));
    bytes
}

fn render(scene: &Scene) -> Vec<u8> {
    frame(&mut engine(scene), scene.version())
}

fn px(frame: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    frame[i..i + 4].try_into().unwrap()
}

fn near(a: [u8; 4], b: [u8; 4], tolerance: i32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= tolerance)
}

const RED: Color = Color::rgb(255, 0, 0);
const BLUE: Color = Color::rgb(0, 0, 255);

fn stops(pairs: &[(f32, Color)]) -> Vec<GradientStop> {
    pairs
        .iter()
        .map(|(p, c)| GradientStop::new(*p, *c))
        .collect()
}

/// A 100×40 box at (20, 20) with the gradient (and extra style).
fn gradient_scene(gradient: Gradient, extra: impl Fn(Style) -> Style) -> Scene {
    Scene::new(
        root().child(Node::container(NodeId(2)).with_style(extra(
            Style::default()
                .size(Length::Px(100.0), Length::Px(40.0))
                .background_gradient(Some(gradient)),
        ))),
    )
}

// ---------------------------------------------------------------- E6

#[test]
fn e6_linear_direction_follows_the_css_angle() {
    let red_blue = stops(&[(0.0, RED), (1.0, BLUE)]);
    // 90deg: towards the right.
    let f = render(&gradient_scene(Gradient::linear(90.0, red_blue.clone()), |s| s));
    assert!(near(px(&f, 20, 40), [255, 0, 0, 255], 8), "{:?}", px(&f, 20, 40));
    assert!(near(px(&f, 119, 40), [0, 0, 255, 255], 8), "{:?}", px(&f, 119, 40));
    let row: Vec<u8> = (20..120).map(|x| px(&f, x, 40)[0]).collect();
    assert!(row.windows(2).all(|p| p[0] >= p[1]), "red falls to the right: {row:?}");
    assert!(near(px(&f, 70, 40), [128, 0, 127, 255], 6), "{:?}", px(&f, 70, 40));
    // Uniform down a column for a horizontal gradient.
    assert_eq!(px(&f, 50, 21), px(&f, 50, 58));
    // Outside the box: untouched.
    assert_eq!(px(&f, 125, 40), BG);
    // 180deg: towards the bottom; 0deg: towards the top.
    let f = render(&gradient_scene(Gradient::linear(180.0, red_blue.clone()), |s| s));
    assert!(near(px(&f, 70, 20), [255, 0, 0, 255], 16));
    assert!(near(px(&f, 70, 59), [0, 0, 255, 255], 16));
    let f = render(&gradient_scene(Gradient::linear(0.0, red_blue), |s| s));
    assert!(near(px(&f, 70, 20), [0, 0, 255, 255], 16));
    assert!(near(px(&f, 70, 59), [255, 0, 0, 255], 16));
}

#[test]
fn e6_stop_positions_and_hard_stops() {
    let hard = stops(&[(0.0, RED), (0.3, RED), (0.3, BLUE), (1.0, BLUE)]);
    let f = render(&gradient_scene(Gradient::linear(90.0, hard), |s| s));
    // The hard edge sits 30 % along: x = 20 + 30.
    assert_eq!(px(&f, 47, 40), [255, 0, 0, 255]);
    assert_eq!(px(&f, 53, 40), [0, 0, 255, 255]);
    // A middle stop pins its colour at its position.
    let three = stops(&[(0.0, RED), (0.75, Color::rgb(0, 255, 0)), (1.0, BLUE)]);
    let f = render(&gradient_scene(Gradient::linear(90.0, three), |s| s));
    assert!(near(px(&f, 95, 40), [0, 255, 0, 255], 12), "{:?}", px(&f, 95, 40));
}

#[test]
fn e6_radial_centre_and_radius() {
    let g = Gradient::circle(0.25, 0.5, 15.0, stops(&[(0.0, RED), (1.0, BLUE)]));
    let f = render(&gradient_scene(g, |s| s));
    // Centre at (20 + 25, 20 + 20).
    assert!(near(px(&f, 45, 40), [255, 0, 0, 255], 24), "{:?}", px(&f, 45, 40));
    // Symmetric around the centre (pixel centres 6.5 px away), last colour beyond the radius.
    assert!(near(px(&f, 38, 40), px(&f, 51, 40), 3));
    assert!(near(px(&f, 45, 33), px(&f, 45, 46), 3));
    assert!(near(px(&f, 38, 40), px(&f, 45, 33), 3));
    assert_eq!(px(&f, 70, 40), [0, 0, 255, 255]);
    assert_eq!(px(&f, 45, 58), [0, 0, 255, 255]);
    // Farthest-corner ellipse (default) at the centre of the box: corners reach the last stop.
    let g = Gradient::radial(0.5, 0.5, stops(&[(0.0, RED), (1.0, BLUE)]));
    let f = render(&gradient_scene(g, |s| s));
    assert!(near(px(&f, 70, 40), [255, 0, 0, 255], 12));
    assert!(px(&f, 20, 20)[2] > 230, "{:?}", px(&f, 20, 20));
}

#[test]
fn e6_rounded_clip_opacity_and_transform() {
    let red_blue = stops(&[(0.0, RED), (1.0, BLUE)]);
    let f = render(&gradient_scene(Gradient::linear(90.0, red_blue.clone()), |s| {
        s.corner_radius(16.0)
    }));
    assert_eq!(px(&f, 20, 20), BG, "cut corner shows the parent");
    assert_eq!(px(&f, 119, 59), BG);
    assert!(near(px(&f, 21, 40), [255, 0, 0, 255], 8));
    // Over the background colour; opacity applies to the whole node.
    let solid = stops(&[(0.0, RED), (1.0, RED)]);
    let f = render(&gradient_scene(Gradient::linear(90.0, solid), |s| {
        s.background(Color::BLUE).opacity(0.5)
    }));
    assert!(near(px(&f, 70, 40), [255, 127, 127, 255], 3), "{:?}", px(&f, 70, 40));
    // Rotated half a turn: blue on the left.
    let f = render(&gradient_scene(Gradient::linear(90.0, red_blue), |s| {
        s.transform(Transform2D::rotate(180.0))
    }));
    assert!(near(px(&f, 20, 40), [0, 0, 255, 255], 8));
    assert!(near(px(&f, 119, 40), [255, 0, 0, 255], 8));
}

#[test]
fn e6_colour_stops_transition() {
    let rule = Transition::new(AnimatedProperties::GRADIENT, 100.0, Easing::LINEAR);
    let style = |c: Color| {
        Style::default()
            .size(Length::Px(100.0), Length::Px(40.0))
            .background_gradient(Some(Gradient::linear(90.0, stops(&[(0.0, c), (1.0, c)]))))
            .transition(rule)
    };
    let scene = Scene::new(root().child(Node::container(NodeId(2)).with_style(style(RED))));
    let mut e = engine(&scene);
    let v = scene.version();
    assert!(e.animate(OWNER, 0.0).is_some());
    e.try_apply(
        OWNER,
        v,
        v + 1,
        &[ScenePatch::UpdateStyle {
            id: NodeId(2),
            style: style(BLUE),
        }],
    )
    .expect("update");
    e.animate(OWNER, 0.0);
    e.animate(OWNER, 50.0);
    let f = frame(&mut e, v + 1);
    assert!(near(px(&f, 70, 40), [128, 0, 128, 255], 2), "{:?}", px(&f, 70, 40));
    e.animate(OWNER, 100.0);
    let f = frame(&mut e, v + 1);
    assert_eq!(px(&f, 70, 40), [0, 0, 255, 255]);
    assert_eq!(e.animating(OWNER), Some(false));
}

#[test]
fn e5_e6_invalid_values_reject_the_whole_frame() {
    let scene = Scene::new(root().child(Node::container(NodeId(2)).with_style(
        Style::default().size(Length::Px(40.0), Length::Px(40.0)),
    )));
    let v = scene.version();
    let mut e = engine(&scene);
    for bad in [
        Style::default().background_gradient(Some(Gradient::linear(0.0, stops(&[(0.0, RED)])))),
        Style::default().background_gradient(Some(Gradient::linear(
            0.0,
            stops(&[(0.6, RED), (0.4, BLUE)]),
        ))),
        Style::default().background_gradient(Some(Gradient::circle(
            0.5,
            0.5,
            -1.0,
            stops(&[(0.0, RED), (1.0, BLUE)]),
        ))),
        Style::default().backdrop(Some(Backdrop::new(500.0, Color::WHITE))),
        Style::default().backdrop(Some(Backdrop::new(4.0, Color::WHITE).saturate(f32::NAN))),
        Style::default().backdrop(Some(Backdrop::new(4.0, Color::WHITE).stroke(0.0, BLUE))),
    ] {
        assert_eq!(
            e.try_apply(
                OWNER,
                v,
                v + 1,
                &[ScenePatch::UpdateStyle {
                    id: NodeId(2),
                    style: bad
                }]
            ),
            Err(SceneUpdateError::NativeRejected)
        );
    }
}

// ---------------------------------------------------------------- E5

/// A 120×60 panel at (20, 20) whose gradient is black on the left half, white on the right;
/// the glass child (id 3) covers it fully.
fn glass_scene(glass: Style) -> Scene {
    let half = stops(&[
        (0.0, Color::BLACK),
        (0.5, Color::BLACK),
        (0.5, Color::WHITE),
        (1.0, Color::WHITE),
    ]);
    Scene::new(
        root().child(
            Node::container(NodeId(2))
                .with_style(
                    Style::default()
                        .size(Length::Px(120.0), Length::Px(60.0))
                        .background_gradient(Some(Gradient::linear(90.0, half))),
                )
                .child(Node::container(NodeId(3)).with_style(
                    glass.size(Length::Px(120.0), Length::Px(60.0)),
                )),
        ),
    )
}

#[test]
fn e5_blur_smooths_the_edge_behind_the_glass() {
    let sharp = render(&glass_scene(Style::default()));
    assert_eq!(px(&sharp, 70, 50), [0, 0, 0, 255]);
    assert_eq!(px(&sharp, 90, 50), [255, 255, 255, 255]);
    let mut e = engine(&glass_scene(
        Style::default().backdrop(Some(Backdrop::new(4.0, Color::TRANSPARENT))),
    ));
    let v = e_version(&glass_scene(Style::default()));
    let f = frame(&mut e, v);
    let row: Vec<u8> = (60..100).map(|x| px(&f, x, 50)[0]).collect();
    assert!(row.windows(2).all(|p| p[0] <= p[1]), "monotone ramp: {row:?}");
    let ramp = row.iter().filter(|v| (12..243).contains(*v)).count();
    assert!(ramp >= 8, "transition spans pixels: {row:?}");
    assert!((96..=160).contains(&px(&f, 79, 50)[0]) || (96..=160).contains(&px(&f, 80, 50)[0]));
    // Far from the edge the colour is kept (x = 30 is still within reach of the white root left
    // of the panel at x < 20, hence a small tolerance).
    assert!(near(px(&f, 30, 50), [0, 0, 0, 255], 4), "{:?}", px(&f, 30, 50));
    assert_eq!(px(&f, 130, 50), [255, 255, 255, 255]);
    // Outside the glass nothing changes.
    for (x, y) in [(10, 10), (150, 50), (70, 90)] {
        assert_eq!(px(&f, x, y), px(&sharp, x, y));
    }
    let report = e.backdrop_report(OWNER).unwrap();
    assert_eq!(report.blurred, vec![NodeId(3)]);
    assert!(report.degraded.is_empty());
    // A larger radius widens the ramp.
    let mut e = engine(&glass_scene(
        Style::default().backdrop(Some(Backdrop::new(10.0, Color::TRANSPARENT))),
    ));
    let wide = frame(&mut e, v);
    let ramp_wide = (60..100)
        .map(|x| px(&wide, x, 50)[0])
        .filter(|v| (12..243).contains(v))
        .count();
    assert!(ramp_wide > ramp, "{ramp_wide} > {ramp}");
}

fn e_version(scene: &Scene) -> u64 {
    scene.version()
}

#[test]
fn e5_neutral_material_is_pixel_identical() {
    // Blur 0, saturation 1, transparent tint: recording, rewriting and replaying the frame must
    // not change any pixel.
    let plain = render(&glass_scene(Style::default()));
    let glass = render(&glass_scene(
        Style::default().backdrop(Some(Backdrop::new(0.0, Color::TRANSPARENT))),
    ));
    assert_eq!(plain, glass);
}

#[test]
fn e5_tint_and_saturation_values() {
    // Tint over white: half-transparent red.
    let f = render(&glass_scene(
        Style::default().backdrop(Some(Backdrop::new(0.0, Color::new(255, 0, 0, 128)))),
    ));
    assert!(near(px(&f, 130, 50), [255, 127, 127, 255], 2), "{:?}", px(&f, 130, 50));
    assert!(near(px(&f, 30, 50), [128, 0, 0, 255], 2), "{:?}", px(&f, 30, 50));
    // Saturation 0 over a coloured backdrop: CSS luma grey.
    let colour = Scene::new(
        root().child(
            Node::container(NodeId(2))
                .with_style(
                    Style::default()
                        .size(Length::Px(120.0), Length::Px(60.0))
                        .background(Color::rgb(200, 100, 50)),
                )
                .child(Node::container(NodeId(3)).with_style(
                    Style::default()
                        .size(Length::Px(60.0), Length::Px(60.0))
                        .backdrop(Some(Backdrop::new(2.0, Color::TRANSPARENT).saturate(0.0))),
                )),
        ),
    );
    let f = render(&colour);
    let luma = (0.213f32 * 200.0 + 0.715 * 100.0 + 0.072 * 50.0).round() as u8;
    assert!(near(px(&f, 50, 50), [luma, luma, luma, 255], 2), "{:?}", px(&f, 50, 50));
    // The right half (outside the glass) keeps its colour.
    assert_eq!(px(&f, 110, 50), [200, 100, 50, 255]);
    // Saturation 2 makes it more vivid.
    let vivid = Scene::new(
        root().child(
            Node::container(NodeId(2))
                .with_style(
                    Style::default()
                        .size(Length::Px(120.0), Length::Px(60.0))
                        .background(Color::rgb(200, 100, 50)),
                )
                .child(Node::container(NodeId(3)).with_style(
                    Style::default()
                        .size(Length::Px(60.0), Length::Px(60.0))
                        .backdrop(Some(Backdrop::new(0.0, Color::TRANSPARENT).saturate(2.0))),
                )),
        ),
    );
    let p = px(&render(&vivid), 50, 50);
    assert!(p[0] > 200 && p[2] < 50, "{p:?}");
}

#[test]
fn e5_rounded_clip_stroke_and_children_above() {
    let sharp = render(&glass_scene(Style::default()));
    let glass = Node::container(NodeId(3)).with_style(
        Style::default()
            .size(Length::Px(120.0), Length::Px(60.0))
            .corner_radius(20.0)
            .padding(Edges::all(20.0))
            .backdrop(Some(
                Backdrop::new(6.0, Color::new(255, 0, 0, 128)).stroke(2.0, Color::rgb(0, 255, 0)),
            )),
    );
    let scene = Scene::new(
        root().child(
            Node::container(NodeId(2))
                .with_style(
                    Style::default()
                        .size(Length::Px(120.0), Length::Px(60.0))
                        .background_gradient(Some(Gradient::linear(
                            90.0,
                            stops(&[
                                (0.0, Color::BLACK),
                                (0.5, Color::BLACK),
                                (0.5, Color::WHITE),
                                (1.0, Color::WHITE),
                            ]),
                        ))),
                )
                .child(glass.child(Node::container(NodeId(4)).with_style(
                    Style::default()
                        .size(Length::Px(10.0), Length::Px(10.0))
                        .background(Color::BLUE),
                ))),
        ),
    );
    let f = render(&scene);
    // Corner tips outside the radius are untouched.
    assert_eq!(px(&f, 20, 20), px(&sharp, 20, 20));
    assert_eq!(px(&f, 139, 79), px(&sharp, 139, 79));
    // Stroke along the inside of the edge (top edge middle, 1px in).
    assert!(near(px(&f, 80, 21), [0, 255, 0, 255], 40), "{:?}", px(&f, 80, 21));
    // Tinted glass inside.
    assert!(near(px(&f, 130, 50), [255, 127, 127, 255], 3), "{:?}", px(&f, 130, 50));
    // The child paints above the material.
    assert_eq!(px(&f, 45, 45), [30, 110, 220, 255]);
}

#[test]
fn e5_over_budget_degrades_to_the_fallback_and_reports() {
    let style = Style::default().backdrop(Some(
        Backdrop::new(8.0, Color::new(255, 255, 255, 60)).fallback(Color::rgb(242, 242, 247)),
    ));
    let scene = glass_scene(style);
    let mut e = engine(&scene);
    let v = scene.version();
    let blurred = frame(&mut e, v);
    assert_eq!(e.backdrop_report(OWNER).unwrap().blurred, vec![NodeId(3)]);
    assert!(e.set_backdrop_budget(OWNER, BackdropBudget::FALLBACK_ONLY));
    let f = frame(&mut e, v);
    assert_ne!(f, blurred);
    assert_eq!(px(&f, 30, 50), [242, 242, 247, 255]);
    assert_eq!(px(&f, 130, 50), [242, 242, 247, 255]);
    let report = e.backdrop_report(OWNER).unwrap();
    assert!(report.blurred.is_empty());
    assert_eq!(report.degraded, vec![(NodeId(3), BackdropDegradation::NodeBudget)]);
    // Pixel budget: the region (box + 3σ margin) does not fit.
    assert!(e.set_backdrop_budget(
        OWNER,
        BackdropBudget {
            max_nodes: 8,
            max_pixels: 1000,
        }
    ));
    frame(&mut e, v);
    assert_eq!(
        e.backdrop_report(OWNER).unwrap().degraded,
        vec![(NodeId(3), BackdropDegradation::PixelBudget)]
    );
    // Back to the default: blurred again, identical to the first frame.
    assert!(e.set_backdrop_budget(OWNER, BackdropBudget::DEFAULT));
    assert_eq!(frame(&mut e, v), blurred);
    // Without a fallback the degraded material paints its tint.
    let tint_only = glass_scene(
        Style::default().backdrop(Some(Backdrop::new(8.0, Color::new(255, 0, 0, 128)))),
    );
    let mut e = engine(&tint_only);
    assert!(e.set_backdrop_budget(OWNER, BackdropBudget::FALLBACK_ONLY));
    let f = frame(&mut e, tint_only.version());
    assert!(near(px(&f, 130, 50), [255, 127, 127, 255], 2));
    assert!(e.backdrop_report(OWNER + 1).is_none());
    assert!(!e.set_backdrop_budget(OWNER + 1, BackdropBudget::DEFAULT));
}

/// Regression: the budget is a host session setting. Applied frames (Insert, UpdateStyle,
/// Remove) used to rebuild the view with `BackdropBudget::DEFAULT`; it must persist until the
/// host sets it again.
/// 回归：预算是宿主会话级设置。已应用帧（插入、改样式、移除）曾以 `BackdropBudget::DEFAULT` 重建视图；
/// 预算须保持到宿主再次设置。
#[test]
fn e5_budget_persists_across_applied_frames() {
    let glass = |a: u8| {
        Style::default().backdrop(Some(
            Backdrop::new(8.0, Color::new(255, 255, 255, a)).fallback(Color::rgb(242, 242, 247)),
        ))
    };
    let strip = |id: u64| {
        Node::container(NodeId(id)).with_style(glass(60).size(Length::Px(120.0), Length::Px(8.0)))
    };
    let scene = glass_scene(glass(60));
    let mut v = scene.version();
    let mut e = engine(&scene);
    let fallback = [242, 242, 247, 255];
    let degraded = |ids: &[u64]| -> Vec<(NodeId, BackdropDegradation)> {
        ids.iter()
            .map(|id| (NodeId(*id), BackdropDegradation::NodeBudget))
            .collect()
    };
    assert!(e.set_backdrop_budget(OWNER, BackdropBudget::FALLBACK_ONLY));
    frame(&mut e, v);
    assert_eq!(e.backdrop_report(OWNER).unwrap().degraded, degraded(&[3]));
    let mut apply = |e: &mut Engine, patches: &[ScenePatch]| {
        e.try_apply(OWNER, v, v + 1, patches).expect("frame");
        v += 1;
        v
    };

    // Insert: a second material; both still degraded. The report keeps describing the last
    // painted frame until the new one paints.
    let at = apply(
        &mut e,
        &[ScenePatch::Insert {
            parent: NodeId(1),
            index: 1,
            node: strip(4),
        }],
    );
    assert_eq!(e.backdrop_report(OWNER).unwrap().degraded, degraded(&[3]));
    let f = frame(&mut e, at);
    assert_eq!(px(&f, 30, 50), fallback);
    let report = e.backdrop_report(OWNER).unwrap();
    assert!(report.blurred.is_empty(), "{report:?}");
    assert_eq!(report.degraded, degraded(&[3, 4]));

    // UpdateStyle: still the fallback.
    let at = apply(
        &mut e,
        &[ScenePatch::UpdateStyle {
            id: NodeId(3),
            style: glass(90).size(Length::Px(120.0), Length::Px(60.0)),
        }],
    );
    let f = frame(&mut e, at);
    assert_eq!(px(&f, 30, 50), fallback);
    assert_eq!(e.backdrop_report(OWNER).unwrap().degraded, degraded(&[3, 4]));

    // Remove: still the fallback.
    let at = apply(&mut e, &[ScenePatch::Remove { id: NodeId(4) }]);
    let f = frame(&mut e, at);
    assert_eq!(px(&f, 30, 50), fallback);
    let report = e.backdrop_report(OWNER).unwrap();
    assert!(report.blurred.is_empty(), "{report:?}");
    assert_eq!(report.degraded, degraded(&[3]));

    // Setting it again takes over and persists in turn: one blurred material per frame.
    assert!(e.set_backdrop_budget(
        OWNER,
        BackdropBudget {
            max_nodes: 1,
            ..BackdropBudget::DEFAULT
        }
    ));
    for patches in [
        vec![ScenePatch::Insert {
            parent: NodeId(1),
            index: 1,
            node: strip(5),
        }],
        vec![ScenePatch::UpdateStyle {
            id: NodeId(5),
            style: glass(30).size(Length::Px(120.0), Length::Px(8.0)),
        }],
    ] {
        let at = apply(&mut e, &patches);
        let f = frame(&mut e, at);
        assert_ne!(px(&f, 30, 50), fallback);
        let report = e.backdrop_report(OWNER).unwrap();
        assert_eq!(report.blurred, vec![NodeId(3)], "{report:?}");
        assert_eq!(report.degraded, degraded(&[5]));
    }
    // Back to the default: both blurred, across a further frame.
    assert!(e.set_backdrop_budget(OWNER, BackdropBudget::DEFAULT));
    let at = apply(&mut e, &[ScenePatch::Remove { id: NodeId(5) }]);
    frame(&mut e, at);
    let report = e.backdrop_report(OWNER).unwrap();
    assert_eq!(report.blurred, vec![NodeId(3)]);
    assert!(report.degraded.is_empty());
}

#[test]
fn e5_material_follows_the_transform_and_transitions() {
    // Translated glass blurs what is behind its painted position.
    let moved = glass_scene(
        Style::default()
            .backdrop(Some(Backdrop::new(0.0, Color::new(255, 0, 0, 128))))
            .transform(Transform2D::translate(0.0, 10.0)),
    );
    let f = render(&moved);
    // Row 85 is below the panel (white root) but inside the moved glass: tinted white.
    assert!(near(px(&f, 130, 85), [255, 127, 127, 255], 2), "{:?}", px(&f, 130, 85));
    assert_eq!(px(&f, 130, 25), [255, 255, 255, 255], "panel top no longer covered");
    // Tint transition: halfway.
    let rule = Transition::new(AnimatedProperties::BACKDROP, 100.0, Easing::LINEAR);
    let style = |a: u8| {
        Style::default()
            .backdrop(Some(Backdrop::new(0.0, Color::new(255, 0, 0, a))))
            .transition(rule)
            .size(Length::Px(120.0), Length::Px(60.0))
    };
    let scene = glass_scene(style(0));
    let v = scene.version();
    let mut e = engine(&scene);
    e.animate(OWNER, 0.0);
    e.try_apply(
        OWNER,
        v,
        v + 1,
        &[ScenePatch::UpdateStyle {
            id: NodeId(3),
            style: style(200),
        }],
    )
    .expect("update");
    e.animate(OWNER, 0.0);
    e.animate(OWNER, 50.0);
    let f = frame(&mut e, v + 1);
    assert!(near(px(&f, 130, 50), [255, 155, 155, 255], 2), "{:?}", px(&f, 130, 50));
}

/// Frame-time measurement behind `BackdropBudget::DEFAULT` (run with `--ignored` in an
/// optimised build): a 1920×1080 window with a glass toolbar (1920×88) and sidebar (280×992),
/// σ = 30, against the same frame with both materials degraded to their fallback.
/// `BackdropBudget::DEFAULT` 背后的帧耗时测量（优化构建下以 `--ignored` 运行）。
#[test]
#[ignore]
fn e5_frame_cost_measurement() {
    const FW: u32 = 1920;
    const FH: u32 = 1080;
    let content = Gradient::linear(
        135.0,
        stops(&[(0.0, Color::rgb(30, 110, 220)), (0.5, Color::WHITE), (1.0, Color::rgb(220, 50, 47))]),
    );
    let glass = |w: f32, h: f32| {
        Style::default()
            .size(Length::Px(w), Length::Px(h))
            .backdrop(Some(
                Backdrop::new(30.0, Color::new(255, 255, 255, 140))
                    .saturate(1.8)
                    .fallback(Color::rgb(242, 242, 247)),
            ))
    };
    let mut body = Node::container(NodeId(10)).with_style(
        Style::default()
            .size(Length::Px(1640.0), Length::Px(992.0))
            .background_gradient(Some(content))
            .direction(FlexDirection::Row)
            .gap(12.0),
    );
    for i in 0..40 {
        body = body.child(Node::container(NodeId(100 + i)).with_style(
            Style::default()
                .size(Length::Px(28.0), Length::Px(900.0))
                .background(Color::rgb((i * 6) as u8, 80, 160))
                .corner_radius(8.0),
        ));
    }
    let scene = Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(FW as f32), Length::Px(FH as f32))
                    .background(Color::WHITE),
            )
            .child(Node::container(NodeId(2)).with_style(glass(FW as f32, 88.0)))
            .child(
                Node::container(NodeId(3))
                    .with_style(Style::default().direction(FlexDirection::Row))
                    .child(Node::container(NodeId(4)).with_style(glass(280.0, 992.0)))
                    .child(body),
            ),
    );
    let mut e = Engine::with_fonts(OWNER, &scene, &[&font()], FW, FH).expect("native import");
    let v = scene.version();
    let mut bytes = vec![0u8; (FW * FH * 4) as usize];
    let mut time = |e: &mut Engine| {
        let mut best = f64::MAX;
        for _ in 0..5 {
            assert!(e.invalidate_frame(OWNER));
            let start = std::time::Instant::now();
            assert!(e.render_to_surface(OWNER, v, &mut bytes, (FW * 4) as usize, 1 << 25));
            best = best.min(start.elapsed().as_secs_f64() * 1000.0);
        }
        best
    };
    let glass_ms = time(&mut e);
    let report = e.backdrop_report(OWNER).unwrap();
    assert_eq!(report.blurred.len(), 2, "{report:?}");
    e.set_backdrop_budget(OWNER, BackdropBudget::FALLBACK_ONLY);
    let fallback_ms = time(&mut e);
    println!(
        "E5 1920x1080: blurred {} px in 2 materials: frame {glass_ms:.1} ms, fallback frame {fallback_ms:.1} ms, material cost {:.1} ms",
        report.pixels,
        glass_ms - fallback_ms
    );
}
