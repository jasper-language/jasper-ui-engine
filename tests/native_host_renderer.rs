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
use jasper_ui_engine::resource::UiBytesSlot;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
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
