//! Built-in-host loopback crossing HostInbox and the native CPU frame path.
//! 经 HostInbox 与原生 CPU 帧路径的内置宿主 loopback。
#![cfg(feature = "native-blitz-cpu-pending")]

use jasper_ui_engine::host_transport::{HostInbox, HostInboxError, HostSessionId};
use jasper_ui_engine::native::host::HostRenderer;
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::style::{Color, Length, Style};

const OWNER: u64 = 701;
const W: u32 = 48;
const H: u32 = 32;
const BUDGET: usize = 32 * 1024;

fn font() -> Vec<u8> {
    std::fs::read(
        std::env::var("JASPER_UI_TEST_FONT")
            .expect("JASPER_UI_TEST_FONT must name an injected font"),
    )
    .expect("injected font readable")
}

fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1)).with_style(
            Style::default()
                .size(Length::Px(W as f32), Length::Px(H as f32))
                .background(Color::RED),
        ),
    )
}

#[derive(Clone, Debug)]
enum Command {
    Apply(Vec<ScenePatch>),
    Render,
    Retire,
}

#[test]
fn ordered_inbox_updates_frame_backpressures_and_retires() {
    let mut source = scene();
    let mut service = HostRenderer::new(OWNER, &source, &font(), W, H).unwrap();
    let generation = service.surface_status(OWNER).unwrap().generation;
    let session = HostSessionId {
        session: 9,
        epoch: 2,
    };
    let mut inbox = HostInbox::new(session, 10, 2).unwrap();
    let patches = vec![ScenePatch::UpdateStyle {
        id: NodeId(1),
        style: Style::default()
            .size(Length::Px(W as f32), Length::Px(H as f32))
            .background(Color::GREEN),
    }];
    let old = source.version();
    source.apply_batch_at(old, &patches).unwrap();
    inbox
        .enqueue(session, 10, Command::Apply(patches.clone()))
        .unwrap();
    inbox.enqueue(session, 11, Command::Render).unwrap();
    assert!(matches!(
        inbox.enqueue(session, 12, Command::Retire),
        Err(HostInboxError::Full(Command::Retire))
    ));

    match inbox.pop().unwrap() {
        Command::Apply(patches) => service
            .try_apply(OWNER, old, source.version(), &patches)
            .unwrap(),
        other => panic!("unexpected first command: {other:?}"),
    }
    let mut pixels = vec![0; (W * H * 4) as usize];
    assert!(matches!(inbox.pop(), Some(Command::Render)));
    assert!(service.render_to_surface(
        OWNER,
        source.version(),
        generation,
        &mut pixels,
        (W * 4) as usize,
        BUDGET
    ));
    let center = ((H as usize / 2) * W as usize + W as usize / 2) * 4;
    assert_eq!(
        pixels[center..center + 4],
        [Color::GREEN.r, Color::GREEN.g, Color::GREEN.b, 255]
    );

    inbox.enqueue(session, 12, Command::Retire).unwrap();
    assert!(matches!(inbox.pop(), Some(Command::Retire)));
    service
        .retire(OWNER, source.version(), generation, &mut |_| {})
        .unwrap();
    let before_retired_render = pixels.clone();
    assert!(!service.render_to_surface(
        OWNER,
        source.version(),
        generation,
        &mut pixels,
        (W * 4) as usize,
        BUDGET
    ));
    assert_eq!(pixels, before_retired_render);
    assert!(inbox.close().is_empty());
}
