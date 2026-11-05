//! End-to-end host lifecycle for injected fonts, software surfaces, DPI, suspend/resume/retire
//! and a failed backend present.
//! 注入字体、软件 Surface、DPI、暂停/恢复/退休和失败 present 的宿主闭环集成测试。
#![cfg(feature = "native-blitz-cpu-pending")]

use jasper_ui_engine::backend::{BackendError, RenderBackend};
use jasper_ui_engine::batch::UiBatch;
use jasper_ui_engine::geometry::Size;
use jasper_ui_engine::native::host::{
    FrameDamage, HostRenderError, HostRenderer, SurfaceLifecycleError, SurfaceState,
};
use jasper_ui_engine::native::{
    EventReply, HostKeyInput, KeyLocation, Modifiers, PointerAction, SceneUpdateError,
};
use jasper_ui_engine::resource::UiBytesSlot;
use jasper_ui_engine::scene::{Node, NodeId, Scene, ScenePatch};
use jasper_ui_engine::style::{Color, Length, Style};

const OWNER: u64 = 71;
const BUDGET: usize = 8 * 1024 * 1024;

fn font() -> Vec<u8> {
    let path = std::env::var("JASPER_UI_TEST_FONT")
        .expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(320.0), Length::Px(200.0))
                    .background(Color::rgb(11, 22, 33)),
            )
            .child(
                Node::rect(NodeId(2)).with_style(
                    Style::default()
                        .size(Length::Px(40.0), Length::Px(30.0))
                        .background(Color::RED),
                ),
            ),
    )
}

struct FailingPresent;

impl RenderBackend for FailingPresent {
    fn resize(&mut self, _size: Size) {}

    fn submit(&mut self, _batch: &UiBatch) -> Result<(), BackendError> {
        Ok(())
    }

    fn submit_rgba8(
        &mut self,
        _width: u32,
        _height: u32,
        _pixels: Vec<u8>,
    ) -> Result<(), BackendError> {
        Ok(())
    }

    fn present(&mut self) -> Result<(), BackendError> {
        Err(BackendError::Io("simulated host present failure".into()))
    }
}

#[test]
fn resource_extent_dpi_pause_resume_and_retirement_are_one_closed_lifecycle() {
    let mut slot = UiBytesSlot::new(OWNER, 1);
    let font_bytes = font();
    let expected_font_len = font_bytes.len();
    let key = slot
        .install(OWNER, font_bytes)
        .expect("font resource issued");
    let source = scene();
    let mut host = {
        let bytes = slot.bytes(OWNER, key).expect("live font bytes");
        HostRenderer::with_fonts(OWNER, &source, &[bytes], 320, 200).expect("native host view")
    };
    // Native construction consumed the bytes through the font stack; the host may now release
    // its external resource without invalidating the already constructed view.
    assert_eq!(slot.release(OWNER, key).unwrap().len(), expected_font_len);
    assert!(slot.bytes(OWNER, key).is_none());
    assert!(slot.release(OWNER, key).is_none());
    assert!(HostRenderer::with_fonts(OWNER, &source, &[], 320, 200).is_none());
    assert!(HostRenderer::with_fonts(OWNER, &source, &[b"not a font"], 320, 200).is_none());
    assert!(HostRenderer::new(OWNER, &source, &font(), 0, 200).is_none());

    let initial = host.surface_status(OWNER).expect("owner status");
    assert_eq!(initial.state, SurfaceState::Active);
    assert_eq!(
        (initial.generation, initial.width, initial.height),
        (1, 320, 200)
    );
    assert_eq!(host.surface_status(OWNER + 1), None);
    assert_eq!(
        host.reconfigure(OWNER, initial.generation, 640, 360, f32::NAN),
        Err(HostRenderError::ViewportRejected)
    );
    assert_eq!(
        host.reconfigure(OWNER, initial.generation, 0, 360, 1.0),
        Err(HostRenderError::ViewportRejected)
    );
    assert_eq!(host.surface_status(OWNER), Some(initial));

    let mut first = vec![0; 320 * 200 * 4];
    assert!(!host.render_to_surface(
        OWNER + 1,
        source.version(),
        initial.generation,
        &mut first,
        320 * 4,
        BUDGET,
    ));
    assert!(first.iter().all(|&byte| byte == 0));
    assert!(host.render_to_surface(
        OWNER,
        source.version(),
        initial.generation,
        &mut first,
        320 * 4,
        BUDGET,
    ));
    assert_eq!(
        &first[..4],
        &[Color::RED.r, Color::RED.g, Color::RED.b, Color::RED.a]
    );

    let resized_generation = host
        .reconfigure(OWNER, initial.generation, 640, 360, 2.0)
        .expect("extent and DPI reconfigure");
    assert!(resized_generation > initial.generation);
    let resized = host.surface_status(OWNER).unwrap();
    assert_eq!((resized.width, resized.height), (640, 360));
    assert!(!host.render_to_surface(
        OWNER,
        source.version(),
        initial.generation,
        &mut first,
        320 * 4,
        BUDGET,
    ));
    let mut scaled = vec![0; 640 * 360 * 4];
    assert!(host.render_to_surface(
        OWNER,
        source.version(),
        resized_generation,
        &mut scaled,
        640 * 4,
        BUDGET,
    ));

    let mut focus = Vec::new();
    let suspended_generation = host
        .suspend(OWNER, source.version(), resized_generation, &mut |event| {
            focus.push(event)
        })
        .expect("host suspend");
    assert!(suspended_generation > resized_generation);
    assert_eq!(
        host.surface_status(OWNER).unwrap().state,
        SurfaceState::Suspended
    );
    assert_eq!(host.needs_frame(OWNER, suspended_generation), None);
    assert!(!host.render_to_surface(
        OWNER,
        source.version(),
        suspended_generation,
        &mut scaled,
        640 * 4,
        BUDGET,
    ));
    assert_eq!(
        host.resume(OWNER, resized_generation, 320, 200, 1.0),
        Err(HostRenderError::Lifecycle(
            SurfaceLifecycleError::StaleGeneration
        ))
    );
    let resumed_generation = host
        .resume(OWNER, suspended_generation, 320, 200, 1.0)
        .expect("host resume");
    assert!(resumed_generation > suspended_generation);
    assert_eq!(host.needs_frame(OWNER, resumed_generation), Some(true));
    let mut resumed = vec![0; 320 * 200 * 4];
    assert!(host.render_to_surface(
        OWNER,
        source.version(),
        resumed_generation,
        &mut resumed,
        320 * 4,
        BUDGET,
    ));

    host.retire(OWNER, source.version(), resumed_generation, &mut |_| {})
        .expect("host retirement");
    assert_eq!(
        host.surface_status(OWNER).unwrap().state,
        SurfaceState::Retired
    );
    assert!(host.snapshot_identity(OWNER).is_none());
    let retired_generation = host.surface_status(OWNER).unwrap().generation;
    assert_eq!(
        host.resume(OWNER, retired_generation, 320, 200, 1.0),
        Err(HostRenderError::Lifecycle(
            SurfaceLifecycleError::WrongState
        ))
    );
}

#[test]
fn failed_backend_present_keeps_the_view_dirty_for_retry() {
    let source = scene();
    let mut host = HostRenderer::new(OWNER, &source, &font(), 320, 200).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    assert_eq!(host.needs_frame(OWNER, generation), Some(true));
    assert!(host
        .render_to_backend(
            OWNER,
            source.version(),
            generation,
            &mut FailingPresent,
            BUDGET,
        )
        .is_err());
    assert_eq!(
        host.needs_frame(OWNER, generation),
        Some(true),
        "a submit that was presented unsuccessfully must remain retryable"
    );
}

#[test]
fn same_extent_dpi_reconfigure_invalidates_the_host_pixel_copy() {
    let source = scene();
    let mut host = HostRenderer::new(OWNER, &source, &font(), 64, 64).unwrap();
    let first_generation = host.surface_status(OWNER).unwrap().generation;
    let mut bytes = vec![0; 64 * 64 * 4];
    assert_eq!(
        host.render_to_surface_with_damage(
            OWNER,
            source.version(),
            first_generation,
            &mut bytes,
            64 * 4,
            BUDGET,
        ),
        Some(FrameDamage::Full)
    );
    assert_eq!(
        host.render_to_surface_with_damage(
            OWNER,
            source.version(),
            first_generation,
            &mut bytes,
            64 * 4,
            BUDGET,
        ),
        Some(FrameDamage::Unchanged)
    );
    let next_generation = host
        .reconfigure(OWNER, first_generation, 64, 64, 2.0)
        .expect("same-extent DPI reconfigure");
    assert_eq!(
        host.render_to_surface_with_damage(
            OWNER,
            source.version(),
            next_generation,
            &mut bytes,
            64 * 4,
            BUDGET,
        ),
        Some(FrameDamage::Full),
        "the newly acquired host Surface needs the complete frame"
    );
}

#[test]
fn every_step_refuses_as_the_lifecycle_does_before_the_view_is_touched() {
    let source = scene();
    let mut host = HostRenderer::new(OWNER, &source, &font(), 320, 200).unwrap();
    let version = source.version();
    let active = host.surface_status(OWNER).unwrap();
    let mut focus_events = 0;
    let mut count = |_| focus_events += 1;

    // A stale generation on a surface that is not suspended is stale first, as in the lifecycle.
    // 非暂停 Surface 上的过期代数先报过期，与生命周期一致。
    assert_eq!(
        host.resume(OWNER, active.generation + 1, 320, 200, 1.0),
        Err(HostRenderError::Lifecycle(
            SurfaceLifecycleError::StaleGeneration
        ))
    );
    assert_eq!(
        host.resume(OWNER, active.generation, 320, 200, 1.0),
        Err(HostRenderError::Lifecycle(
            SurfaceLifecycleError::WrongState
        ))
    );
    assert_eq!(
        host.suspend(OWNER + 1, version, active.generation, &mut count),
        Err(HostRenderError::Lifecycle(
            SurfaceLifecycleError::WrongOwner
        ))
    );
    assert_eq!(host.surface_status(OWNER), Some(active));
    assert!(host.snapshot_identity(OWNER).is_some());

    // The engine's refusal of a stale version leaves the lifecycle where it was.
    // 引擎拒绝过期版本时，生命周期保持原状。
    assert_eq!(
        host.suspend(OWNER, version + 1, active.generation, &mut count),
        Err(HostRenderError::ViewRetired)
    );
    assert_eq!(host.surface_status(OWNER), Some(active));
    assert_eq!(
        host.retire(OWNER, version + 1, active.generation, &mut count),
        Err(HostRenderError::ViewRetired)
    );
    assert_eq!(host.surface_status(OWNER), Some(active));
    assert!(!host.close(OWNER, version + 1, &mut count));
    assert_eq!(host.surface_status(OWNER), Some(active));

    // A suspended surface still retires, and retiring twice is the lifecycle's refusal.
    // 暂停的 Surface 仍可退休；重复退休由生命周期拒绝。
    let suspended = host
        .suspend(OWNER, version, active.generation, &mut count)
        .unwrap();
    host.retire(OWNER, version, suspended, &mut count).unwrap();
    let retired = host.surface_status(OWNER).unwrap();
    assert_eq!(retired.state, SurfaceState::Retired);
    assert_eq!(
        host.retire(OWNER, version, retired.generation, &mut count),
        Err(HostRenderError::Lifecycle(
            SurfaceLifecycleError::WrongState
        ))
    );
    assert!(!host.close(OWNER, version, &mut count));
    assert_eq!(host.surface_status(OWNER), Some(retired));
    assert_eq!(focus_events, 0);
}

#[test]
fn a_failed_software_render_leaves_the_view_dirty() {
    let source = scene();
    let version = source.version();
    let mut host = HostRenderer::new(OWNER, &source, &font(), 320, 200).unwrap();
    let generation = host.surface_status(OWNER).unwrap().generation;
    let mut bytes = vec![0; 320 * 200 * 4];
    assert!(host.render_to_surface(OWNER, version, generation, &mut bytes, 320 * 4, BUDGET));
    assert_eq!(host.needs_frame(OWNER, generation), Some(false));
    assert!(!host.render_to_surface(OWNER, version + 1, generation, &mut bytes, 320 * 4, BUDGET));
    assert_eq!(host.needs_frame(OWNER, generation), Some(true));
    assert!(host.render_to_surface(OWNER, version, generation, &mut bytes, 320 * 4, BUDGET));
    assert_eq!(host.needs_frame(OWNER, generation), Some(false));
    assert_eq!(
        host.render_to_surface_with_damage(
            OWNER,
            version + 1,
            generation,
            &mut bytes,
            320 * 4,
            BUDGET
        ),
        None
    );
    assert_eq!(host.needs_frame(OWNER, generation), Some(true));
}

// A patch the engine would accept, so that a refusal can only come from the gate.
// 引擎本会接受的补丁，因此拒绝只可能来自门。
fn accepted_patch() -> [ScenePatch; 1] {
    [ScenePatch::UpdateStyle {
        id: NodeId(2),
        style: Style::default()
            .size(Length::Px(41.0), Length::Px(30.0))
            .background(Color::RED),
    }]
}

// Calls every generation-gated entry with the given owner and generation and checks that it refuses.
// A frame that is clean going in stays clean, so an entry that reached the view would show.
// 以给定归属者和代数调用每个受代数门控的入口并检查其拒绝。进入时干净的帧保持干净，入口若碰到视图就会暴露。
fn assert_every_gated_entry_refuses(
    host: &mut HostRenderer,
    owner: u64,
    version: u64,
    generation: u64,
) {
    let sink_calls = std::cell::Cell::new(0);
    let mut bytes = vec![0; 320 * 200 * 4];
    let key = HostKeyInput {
        key: "Tab",
        code: "Tab",
        pressed: true,
        repeat: false,
        composing: false,
        modifiers: Modifiers::default(),
        location: KeyLocation::Standard,
    };
    assert_eq!(host.needs_frame(owner, generation), None);
    assert!(host
        .accessibility_tree(owner, version, generation)
        .is_none());
    assert!(
        !host.focus(owner, version, generation, NodeId(2), &mut |_| sink_calls
            .set(sink_calls.get() + 1))
    );
    assert!(
        !host.deactivate(owner, version, generation, &mut |_| sink_calls
            .set(sink_calls.get() + 1))
    );
    assert!(!host.set_viewport_scroll(owner, version, generation, 0.0, 1.0));
    assert!(!host.invalidate_frame(owner, generation));
    assert!(!host.pointer(
        owner,
        version,
        generation,
        PointerAction::Down,
        5.0,
        5.0,
        true,
        &mut |_| {
            sink_calls.set(sink_calls.get() + 1);
            EventReply::default()
        },
        &mut |_| sink_calls.set(sink_calls.get() + 1),
    ));
    assert!(!host.key(
        owner,
        version,
        generation,
        &key,
        &mut |_| {
            sink_calls.set(sink_calls.get() + 1);
            EventReply::default()
        },
        &mut |_| sink_calls.set(sink_calls.get() + 1),
    ));
    assert!(!host.apply_with_focus_events(
        owner,
        version,
        version + 1,
        generation,
        &accepted_patch(),
        &mut |_| sink_calls.set(sink_calls.get() + 1),
    ));
    assert!(!host.render_to_surface(owner, version, generation, &mut bytes, 320 * 4, BUDGET));
    assert_eq!(
        host.render_to_surface_with_damage(owner, version, generation, &mut bytes, 320 * 4, BUDGET),
        None
    );
    assert!(host
        .render_to_backend(owner, version, generation, &mut FailingPresent, BUDGET)
        .is_err());
    assert_eq!(sink_calls.get(), 0);
    assert!(
        bytes.iter().all(|byte| *byte == 0),
        "a refused render wrote pixels"
    );
}

#[test]
fn every_gated_entry_refuses_before_the_view_is_touched() {
    let source = scene();
    let version = source.version();
    let mut host = HostRenderer::new(OWNER, &source, &font(), 320, 200).unwrap();
    let live = host.surface_status(OWNER).unwrap().generation;
    let mut bytes = vec![0; 320 * 200 * 4];
    assert!(host.render_to_surface(OWNER, version, live, &mut bytes, 320 * 4, BUDGET));
    let identity = host.snapshot_identity(OWNER);

    for (owner, generation) in [(OWNER + 1, live), (OWNER, live + 1), (OWNER, live - 1)] {
        assert_every_gated_entry_refuses(&mut host, owner, version, generation);
        assert_eq!(
            host.needs_frame(OWNER, live),
            Some(false),
            "the clean frame stayed clean"
        );
        assert_eq!(host.snapshot_identity(OWNER), identity);
    }

    // Without a generation, these two still ask the lifecycle who may touch the view.
    // 这两个入口不带代数，仍由生命周期判定谁能碰视图。
    assert!(!host.apply(OWNER + 1, version, version + 1, &accepted_patch()));
    assert!(host
        .try_apply(OWNER + 1, version, version + 1, &accepted_patch())
        .is_err());
    assert!(host.focused_node(OWNER + 1, version).is_none());
    assert_eq!(host.needs_frame(OWNER, live), Some(false));

    let suspended = host.suspend(OWNER, version, live, &mut |_| {}).unwrap();
    assert_every_gated_entry_refuses(&mut host, OWNER, version, suspended);
    assert_eq!(
        host.try_apply(OWNER, version, version + 1, &accepted_patch()),
        Err(SceneUpdateError::Closed)
    );
    assert!(!host.apply(OWNER, version, version + 1, &accepted_patch()));
    assert!(host.focused_node(OWNER, version).is_none());
    assert_eq!(host.snapshot_identity(OWNER), identity);

    host.retire(OWNER, version, suspended, &mut |_| {}).unwrap();
    let retired = host.surface_status(OWNER).unwrap().generation;
    assert_every_gated_entry_refuses(&mut host, OWNER, version, retired);
    assert_eq!(
        host.try_apply(OWNER, version, version + 1, &accepted_patch()),
        Err(SceneUpdateError::Closed)
    );
}
