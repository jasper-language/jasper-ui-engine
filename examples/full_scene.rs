//! Full public caller path for checklist item 5.
//! 清单第 5 项的完整公共调用方路径。
//!
//! Declaration → versioned visual update → layout/presentation → pointer interaction →
//! external text/IME client → retirement. The run also checks focus, viewport scroll and
//! accessibility across the update, rejects resource failures without corrupting the view,
//! and prints a reproducible CPU frame-time baseline.
//!
//! Run:
//! `cargo run --locked --features native-engine-cpu --example full_scene -- FONT [OUTPUT]`

use std::time::{Duration, Instant};

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::host::{TextDisplayView, UiInputBinding, UiInputContext};
use jasper_ui_engine::native::{
    ComponentUpdate, EventReply, PointerAction, SceneEventKind, TextAppearance,
};
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::semantics::{Role, Semantics};
use jasper_ui_engine::style::{Color, Display, FlexDirection, FontStyle, Length, Overflow, Style};

const SCENE_OWNER: u64 = 10;
const TEXT_OWNER: u64 = 20;
const W: u32 = 360;
const H: u32 = 240;
const BUDGET: usize = 4 * 1024 * 1024;
const FRAME_BYTES: usize = (W * H * 4) as usize;
const BUTTON: NodeId = NodeId(2);
const TEXT_TARGET: NodeId = NodeId(9);
const PERF_SAMPLES: usize = 7;

fn fail<T>(message: &str) -> Result<T, Box<dyn std::error::Error>> {
    Err(message.into())
}

fn scene() -> Scene {
    let button = Node::text(BUTTON, "Activate")
        .with_semantics(
            Semantics::default()
                .role(Role::Button)
                .label("Activate")
                .focusable(true),
        )
        .with_style(
            Style::default()
                .size(Length::Px(160.0), Length::Px(48.0))
                .background(Color::rgb(40, 130, 90)),
        );

    let scroll = Node::container(NodeId(3))
        .with_semantics(
            Semantics::default()
                .role(Role::ScrollView)
                .label("Scrollable surface"),
        )
        .with_style(
            Style::default()
                .size(Length::Px(300.0), Length::Px(80.0))
                .display(Display::Block)
                .overflow(Overflow::Hidden),
        )
        .child(
            Node::rect(NodeId(4)).with_style(
                Style::default()
                    .size(Length::Px(300.0), Length::Px(120.0))
                    .background(Color::rgb(180, 54, 48)),
            ),
        );

    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .padding(Edges::all(20.0))
                    .gap(16.0)
                    .direction(FlexDirection::Column)
                    .background(Color::rgb(18, 22, 30)),
            )
            .child(button)
            .child(scroll),
    )
}

fn render_scene(engine: &mut Engine, version: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut pixels = vec![0; FRAME_BYTES];
    if !engine.render_to_surface(SCENE_OWNER, version, &mut pixels, (W * 4) as usize, BUDGET) {
        return fail("native CPU frame rejected");
    }
    Ok(pixels)
}

fn percentile(samples: &[Duration], percent: usize) -> Duration {
    let rank = (samples.len() * percent).div_ceil(100).max(1) - 1;
    samples[rank.min(samples.len() - 1)]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let font_path = arguments.next().ok_or("supply an injected font file")?;
    let output = arguments.next().unwrap_or_else(|| "full_scene.ppm".into());
    if arguments.next().is_some() {
        return fail("expected FONT [OUTPUT]");
    }
    let font = std::fs::read(font_path)?;
    if let Some(parent) = std::path::Path::new(&output)
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut source = scene();
    let mut engine =
        Engine::new(SCENE_OWNER, &source, &font, W, H).ok_or("native scene import rejected")?;
    if Engine::with_fonts(SCENE_OWNER, &source, &[], W, H).is_some() {
        return fail("empty injected font was accepted");
    }

    if !engine.focus(SCENE_OWNER, source.version(), BUTTON, &mut |_| {}) {
        return fail("Framework focus request rejected");
    }
    if !engine.set_viewport_scroll(SCENE_OWNER, source.version(), 0.0, 8.0) {
        return fail("host viewport scroll rejected");
    }
    let accessibility = engine
        .accessibility_tree(SCENE_OWNER, source.version())
        .ok_or("initial accessibility tree rejected")?;
    let accessible_button = accessibility
        .nodes
        .iter()
        .find(|node| node.node == BUTTON)
        .ok_or("button missing from accessibility tree")?;
    if accessibility.focused != Some(BUTTON)
        || accessible_button.label != "Activate"
        || !accessible_button.actions.focus
        || !accessible_button.actions.activate
    {
        return fail("initial accessibility state mismatch");
    }

    let mut activations = Vec::new();
    if !engine.pointer(
        SCENE_OWNER,
        source.version(),
        PointerAction::Down,
        30.0,
        30.0,
        true,
        &mut |_| EventReply::default(),
        &mut |_| {},
    ) {
        return fail("native pointer down rejected");
    }
    let scrolled = render_scene(&mut engine, source.version())?;
    if scrolled
        .chunks_exact(4)
        .all(|pixel| pixel == &scrolled[..4])
    {
        return fail("scene produced a constant test frame");
    }

    // This frame changes only visual style. Focus, the pressed gesture and the host viewport
    // scroll must survive the native whole-frame rebuild.
    let old = source.version();
    let patches = [ScenePatch::UpdateStyle {
        id: BUTTON,
        style: Style::default()
            .size(Length::Px(160.0), Length::Px(48.0))
            .background(Color::rgb(48, 160, 105))
            .font(700, FontStyle::Normal),
    }];
    source
        .apply_batch_at(old, &patches)
        .map_err(|error| format!("source update rejected: {error:?}"))?;
    let mut focus_changes = Vec::new();
    if !engine.apply_with_focus_events(SCENE_OWNER, old, source.version(), &patches, &mut |event| {
        focus_changes.push((event.node, event.focused))
    }) {
        return fail("visual SceneUpdate rejected");
    }
    if !focus_changes.is_empty()
        || engine.focused_node(SCENE_OWNER, source.version()) != Some(BUTTON)
    {
        return fail("visual update lost focus");
    }
    let accessibility = engine
        .accessibility_tree(SCENE_OWNER, source.version())
        .ok_or("updated accessibility tree rejected")?;
    if accessibility.focused != Some(BUTTON)
        || !accessibility
            .nodes
            .iter()
            .any(|node| node.node == BUTTON && node.actions.activate)
    {
        return fail("visual update lost accessibility state");
    }

    let updated = render_scene(&mut engine, source.version())?;
    if updated == scrolled {
        return fail("visual update did not change presentation");
    }
    // A fresh import with the same source and host scroll is the independent reference for
    // the incremental frame. Equality also proves the pure visual update retained scroll.
    let mut reference =
        Engine::new(SCENE_OWNER, &source, &font, W, H).ok_or("reference native scene rejected")?;
    if !reference.set_viewport_scroll(SCENE_OWNER, source.version(), 0.0, 8.0) {
        return fail("reference viewport scroll rejected");
    }
    if render_scene(&mut reference, source.version())? != updated {
        return fail("incremental visual frame differs from scrolled fresh import");
    }

    if !engine.pointer(
        SCENE_OWNER,
        source.version(),
        PointerAction::Up,
        30.0,
        30.0,
        false,
        &mut |event| {
            if event.kind == SceneEventKind::Activate {
                activations.push(event.node);
            }
            EventReply::default()
        },
        &mut |_| {},
    ) {
        return fail("native pointer up rejected");
    }
    if activations != [BUTTON] {
        return fail("visual update/scroll sequence changed activation");
    }

    // Small surfaces, stale versions and over-budget frames fail before touching host storage.
    let guard = [0xA5; 33];
    let mut guarded = guard;
    if engine.render_to_surface(
        SCENE_OWNER,
        source.version(),
        &mut guarded,
        (W * 4) as usize,
        1,
    ) || guarded != guard
    {
        return fail("resource failure modified host surface");
    }
    if engine.render_to_surface(
        SCENE_OWNER,
        source.version() + 1,
        &mut guarded,
        (W * 4) as usize,
        BUDGET,
    ) {
        return fail("stale frame was accepted");
    }

    let mut backend = HeadlessBackend::new(1, 1);
    engine
        .render_to_backend(SCENE_OWNER, source.version(), &mut backend, BUDGET)
        .map_err(|error| format!("CPU presentation rejected: {error:?}"))?;
    backend.save_ppm(&output)?;

    // The window/text client receives authoritative snapshots and transient preedit text.
    // It is deliberately a synchronous injected-service fixture, not an editor or IME engine.
    let context = UiInputContext {
        target: TEXT_TARGET,
        target_generation: 1,
        document: 2,
        revision: 3,
    };
    let next_context = UiInputContext {
        revision: 4,
        ..context
    };
    let mut display = TextDisplayView::new(
        TEXT_OWNER,
        context,
        &font,
        "External service document",
        W,
        H,
    );
    let initial_display = display
        .render_cpu(TEXT_OWNER, context, BUDGET)
        .ok_or("initial text frame rejected")?;
    if !display.update(
        TEXT_OWNER,
        context,
        0,
        &[
            ComponentUpdate::Appearance(TextAppearance {
                font_size_px: 22,
                padding_px: 10,
                foreground: [236, 240, 248],
                background: [18, 22, 30],
            }),
            ComponentUpdate::AccessibleLabel(Some("External document")),
        ],
    ) {
        return fail("external display update rejected");
    }
    let mut binding = UiInputBinding::new(TEXT_OWNER, 1);
    let ticket = binding
        .bind(TEXT_OWNER, context)
        .ok_or("text binding rejected")?;
    if !display.focus(&binding, TEXT_OWNER, ticket, &mut |_| {}) {
        return fail("text focus rejected");
    }
    if !display.snapshot(
        TEXT_OWNER,
        context,
        next_context,
        "External service successor",
    ) {
        return fail("external display snapshot rejected");
    }
    let successor_display = display
        .render_cpu(TEXT_OWNER, next_context, BUDGET)
        .ok_or("successor text frame rejected")?;
    if successor_display == initial_display {
        return fail("external display successor did not repaint");
    }
    let successor = binding
        .replace(TEXT_OWNER, ticket, next_context)
        .ok_or("binding rotation rejected")?;
    let token = display
        .begin_preedit(&binding, TEXT_OWNER, successor)
        .ok_or("preedit begin rejected")?;
    if !display.preedit(&binding, TEXT_OWNER, token, Some("compose")) {
        return fail("preedit update rejected");
    }
    let composed = display
        .render_cpu(TEXT_OWNER, next_context, BUDGET)
        .ok_or("preedit frame rejected")?;
    if composed == successor_display {
        return fail("preedit did not repaint");
    }
    if !display.cancel_preedit(TEXT_OWNER, token)
        || display
            .render_cpu(TEXT_OWNER, next_context, BUDGET)
            .as_ref()
            != Some(&successor_display)
    {
        return fail("preedit cancellation did not restore display");
    }
    if display.semantics(TEXT_OWNER, next_context).is_none() {
        return fail("external display accessibility semantics rejected");
    }
    if !display.close_focus(&mut binding, TEXT_OWNER, successor, &mut |_| {}) {
        return fail("text focus lease close rejected");
    }
    if !display.close(TEXT_OWNER, next_context) {
        return fail("text view retirement rejected");
    }

    // Warm one frame, then force real paints. This is a regression baseline for this 360x240
    // reference scene on the executing VM; it is not a hardware/display or GPU benchmark.
    let mut samples = Vec::with_capacity(PERF_SAMPLES);
    let mut perf_surface = vec![0; FRAME_BYTES];
    for index in 0..PERF_SAMPLES {
        if !engine.invalidate_frame(SCENE_OWNER) {
            return fail("frame invalidation rejected");
        }
        let started = Instant::now();
        if !engine.render_to_surface(
            SCENE_OWNER,
            source.version(),
            &mut perf_surface,
            (W * 4) as usize,
            BUDGET,
        ) {
            return fail("baseline frame rejected");
        }
        if index > 0 {
            samples.push(started.elapsed());
        }
    }
    samples.sort_unstable();
    let median = percentile(&samples, 50);
    let p95 = percentile(&samples, 95);

    if !engine.focus(SCENE_OWNER, source.version(), BUTTON, &mut |_| {}) {
        return fail("pre-retirement focus request rejected");
    }
    let mut retired_focus = false;
    if !engine.close_with_focus_events(SCENE_OWNER, source.version(), &mut |event| {
        retired_focus = event.node == BUTTON && !event.focused;
    }) {
        return fail("scene retirement rejected");
    }
    if !retired_focus {
        return fail("scene retirement omitted focus loss");
    }
    if engine
        .accessibility_tree(SCENE_OWNER, source.version())
        .is_some()
    {
        return fail("accessibility tree survived retirement");
    }
    if engine.snapshot_identity(SCENE_OWNER).is_some() {
        return fail("scene identity survived retirement");
    }

    println!(
        "item5_full_scene=pass output={output} scene={W}x{H} update_focus=preserved \
scroll=preserved accessibility=passed resource_failures=passed text_ime=passed"
    );
    println!(
        "item5_perf scene_cpu_scroll_patch median_us={} p95_us={} samples={} \
surface_bytes={} viewport={W}x{H}",
        median.as_micros(),
        p95.as_micros(),
        samples.len(),
        perf_surface.len()
    );
    Ok(())
}
