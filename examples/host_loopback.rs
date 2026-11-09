//! Headless system-built-in-host loopback without winit or wgpu.
//! 无 winit/wgpu 的内置系统宿主 loopback。
//!
//! The bounded inbox models the ordering/backpressure boundary between an application-side
//! producer and a system UI service. It is a reference integration, not a system IPC implementation.
//! 有界收件队列模拟应用侧生产者与系统 UI 服务之间的顺序/背压边界；这是集成参考，不是系统 IPC。
//!
//! Run:
//! `cargo run --locked --features native-engine-cpu --example host_loopback -- FONT [OUTPUT.ppm]`

#[cfg(feature = "native-engine-cpu")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use jasper_ui_engine::host_transport::{HostInbox, HostInboxError, HostSessionId};
    use jasper_ui_engine::native::host::HostRenderer;
    use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
    use jasper_ui_engine::style::{Color, Length, Style};

    const OWNER: u64 = 700;
    const WIDTH: u32 = 64;
    const HEIGHT: u32 = 64;
    const BUDGET: usize = 64 * 1024;

    #[derive(Clone, Debug)]
    enum Command {
        Apply {
            expected: u64,
            next: u64,
            patches: Vec<ScenePatch>,
        },
        Render,
        Retire,
    }

    fn scene() -> Scene {
        Scene::new(
            Node::container(NodeId(1)).with_style(
                Style::default()
                    .size(Length::Px(WIDTH as f32), Length::Px(HEIGHT as f32))
                    .background(Color::RED),
            ),
        )
    }

    fn write_ppm(path: &str, width: u32, height: u32, pixels: &[u8]) -> std::io::Result<()> {
        let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
        ppm.extend(pixels.chunks_exact(4).flat_map(|pixel| &pixel[..3]));
        std::fs::write(path, ppm)
    }

    let mut arguments = std::env::args().skip(1);
    let font_path = arguments.next().ok_or("supply an injected font file")?;
    let output = arguments
        .next()
        .unwrap_or_else(|| "host_loopback.ppm".into());
    if arguments.next().is_some() {
        return Err("expected FONT [OUTPUT.ppm]".into());
    }
    let font = std::fs::read(font_path)?;
    let mut source = scene();
    let mut service =
        HostRenderer::new(OWNER, &source, &font, WIDTH, HEIGHT).ok_or("native service import")?;
    let generation = service
        .surface_status(OWNER)
        .ok_or("missing service surface")?
        .generation;
    let session = HostSessionId {
        session: 41,
        epoch: 3,
    };
    let mut inbox = HostInbox::new(session, 1, 2).ok_or("zero inbox capacity")?;
    let old = source.version();
    let patches = vec![ScenePatch::UpdateStyle {
        id: NodeId(1),
        style: Style::default()
            .size(Length::Px(WIDTH as f32), Length::Px(HEIGHT as f32))
            .background(Color::GREEN),
    }];
    source
        .apply_batch_at(old, &patches)
        .map_err(|error| format!("source update rejected: {error:?}"))?;
    inbox
        .enqueue(
            session,
            1,
            Command::Apply {
                expected: old,
                next: source.version(),
                patches,
            },
        )
        .map_err(|error| format!("inbox rejected update: {error:?}"))?;
    inbox
        .enqueue(session, 2, Command::Render)
        .map_err(|error| format!("inbox rejected render: {error:?}"))?;
    assert!(matches!(
        inbox.enqueue(session, 3, Command::Retire),
        Err(HostInboxError::Full(Command::Retire))
    ));

    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    let mut retired = false;
    while let Some(command) = inbox.pop() {
        match command {
            Command::Apply {
                expected,
                next,
                patches,
            } => service
                .try_apply(OWNER, expected, next, &patches)
                .map_err(|error| format!("scene update rejected: {error:?}"))?,
            Command::Render => {
                if !service.render_to_surface(
                    OWNER,
                    source.version(),
                    generation,
                    &mut pixels,
                    (WIDTH * 4) as usize,
                    BUDGET,
                ) {
                    return Err("CPU frame rejected".into());
                }
            }
            Command::Retire => {}
        }
    }
    inbox
        .enqueue(session, 3, Command::Retire)
        .map_err(|error| format!("inbox rejected retirement: {error:?}"))?;
    if let Some(Command::Retire) = inbox.pop() {
        service.retire(OWNER, source.version(), generation, &mut |_| {})?;
        retired = true;
    }
    if !retired {
        return Err("retirement command was not consumed".into());
    }
    if service.render_to_surface(
        OWNER,
        source.version(),
        generation,
        &mut pixels,
        (WIDTH * 4) as usize,
        BUDGET,
    ) {
        return Err("retirement did not close the service".into());
    }
    write_ppm(&output, WIDTH, HEIGHT, &pixels)?;
    let center = ((HEIGHT as usize / 2) * WIDTH as usize + WIDTH as usize / 2) * 4;
    if pixels[center..center + 4] != [Color::GREEN.r, Color::GREEN.g, Color::GREEN.b, 255] {
        return Err("loopback frame does not contain the applied scene".into());
    }
    println!(
        "host_loopback=pass session={} epoch={} version={} output={output}",
        session.session,
        session.epoch,
        source.version()
    );
    Ok(())
}

#[cfg(not(feature = "native-engine-cpu"))]
fn main() {
    eprintln!("host_loopback requires --features native-engine-cpu");
}
