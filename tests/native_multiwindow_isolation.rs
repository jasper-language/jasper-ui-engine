//! Two owner-qualified native views remain independent through resize, suspend and retirement.
//! 两个带 owner 的原生视图在重配、暂停与退休期间保持相互隔离。
#![cfg(feature = "native-blitz-cpu-pending")]

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::native::host::{HostRenderer, SurfaceState};
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, Length, Style};

const BUDGET: usize = 64 * 1024;

fn font() -> Vec<u8> {
    std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected Latin font"),
    )
    .expect("injected font readable")
}

fn scene(color: Color) -> Scene {
    Scene::new(
        Node::container(NodeId(1)).with_style(
            Style::default()
                .size(Length::Px(64.0), Length::Px(64.0))
                .background(color),
        ),
    )
}

fn first_pixel(view: &mut HostRenderer, owner: u64, generation: u64, source: &Scene) -> [u8; 4] {
    let mut backend = HeadlessBackend::new(64, 64);
    view.render_to_backend(owner, source.version(), generation, &mut backend, BUDGET)
        .expect("native frame");
    backend.canvas().pixels()[0..4]
        .try_into()
        .expect("RGBA pixel")
}

#[test]
fn owners_keep_independent_presentation_lifecycles() {
    let bytes = font();
    let red = scene(Color::RED);
    let blue = scene(Color::BLUE);
    let mut a = HostRenderer::new(100, &red, &bytes, 64, 64).expect("owner 100");
    let mut b = HostRenderer::new(101, &blue, &bytes, 64, 64).expect("owner 101");
    let mut ag = a.surface_status(100).unwrap().generation;
    let mut bg = b.surface_status(101).unwrap().generation;

    assert_eq!(first_pixel(&mut a, 100, ag, &red), [220, 50, 47, 255]);
    assert_eq!(first_pixel(&mut b, 101, bg, &blue), [30, 110, 220, 255]);

    ag = a.reconfigure(100, ag, 32, 32, 1.0).unwrap();
    assert_eq!(a.surface_status(100).unwrap().state, SurfaceState::Active);
    assert_eq!(
        b.surface_status(101).unwrap(),
        jasper_ui_engine::native::host::SurfaceStatus {
            state: SurfaceState::Active,
            generation: bg,
            width: 64,
            height: 64,
        }
    );
    assert_eq!(first_pixel(&mut b, 101, bg, &blue), [30, 110, 220, 255]);

    let suspended = a.suspend(100, red.version(), ag, &mut |_| {}).unwrap();
    assert_eq!(
        a.surface_status(100).unwrap().state,
        SurfaceState::Suspended
    );
    assert_eq!(first_pixel(&mut b, 101, bg, &blue), [30, 110, 220, 255]);

    ag = a.resume(100, suspended, 40, 40, 1.0).unwrap();
    assert_eq!(first_pixel(&mut a, 100, ag, &red), [220, 50, 47, 255]);
    a.retire(100, red.version(), ag, &mut |_| {}).unwrap();
    assert_eq!(a.surface_status(100).unwrap().state, SurfaceState::Retired);

    bg = b.reconfigure(101, bg, 48, 48, 1.0).unwrap();
    assert_eq!(first_pixel(&mut b, 101, bg, &blue), [30, 110, 220, 255]);
}
