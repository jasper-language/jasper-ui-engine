#![no_main]
use jasper_ui_host_surface_verification::{SurfaceLifecycle, SurfaceLifecycleError, SurfaceState};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let owner = data
        .get(..8)
        .map_or(0, |bytes| u64::from_le_bytes(bytes.try_into().unwrap()));
    let mut surface = SurfaceLifecycle::new(owner, 320, 200);
    for byte in data.iter().copied() {
        let before = surface.status(owner).unwrap();
        let generation = before.generation;
        let transition = match byte % 7 {
            0 => surface.reconfigure(owner, generation, 640, 360),
            1 => surface.suspend(owner, generation),
            2 => surface.resume(owner, generation, 320, 200),
            3 => surface.retire(owner, generation).map(|_| generation),
            4 => surface.reconfigure(owner.wrapping_add(1), generation, 1, 1),
            5 => surface.suspend(owner, generation.wrapping_add(1)),
            _ => surface.resume(owner, generation.wrapping_add(1), 1, 1),
        };
        let after = surface.status(owner).unwrap();
        assert_eq!(surface.status(owner.wrapping_add(1)), None);
        assert_eq!(
            surface.render_allowed(owner, after.generation),
            after.state == SurfaceState::Active
        );
        match transition {
            Ok(_) => assert!(after.generation >= generation),
            Err(error) => {
                assert_eq!(after, before, "rejected {error:?} changed lifecycle state");
                assert!(!matches!(error, SurfaceLifecycleError::GenerationExhausted));
            }
        }
    }
});
