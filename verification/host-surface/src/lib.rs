//! Executes the production surface lifecycle source extracted by `build.rs`.
//! 执行由 `build.rs` 提取的生产 Surface 生命周期源码。
mod actual {
    include!(concat!(env!("OUT_DIR"), "/actual.rs"));
}
pub use actual::*;

/// The production capability matrix, for its table tests. No Cargo feature is on in this crate.
/// 生产能力矩阵，用于判定表测试。本 crate 不开任何 Cargo feature。
pub mod capabilities {
    include!(concat!(env!("OUT_DIR"), "/capabilities.rs"));
}

/// Reaches the crate-private compatibility retirement for the fuzz target, unchanged.
/// 让 fuzz 目标原样调用 crate 内部的兼容退休路径。
pub fn retire_current(surface: &mut SurfaceLifecycle, caller: u64) -> bool {
    surface.retire_current(caller)
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn successful_lifecycle_is_monotone_and_final() {
        let owner: u64 = kani::any();
        let mut surface = SurfaceLifecycle::new(owner, kani::any(), kani::any());
        let first = surface.reconfigure(owner, 1, kani::any(), kani::any()).unwrap();
        kani::assert(first > 1, "reconfigure must advance");
        let suspended = surface.suspend(owner, first).unwrap();
        kani::assert(suspended > first, "suspend must advance");
        let resumed = surface
            .resume(owner, suspended, kani::any(), kani::any())
            .unwrap();
        kani::assert(resumed > suspended, "resume must advance");
        kani::assert(
            surface.render_allowed(owner, resumed),
            "resumed generation must be active",
        );
        surface.retire(owner, resumed).unwrap();
        kani::assert(
            !surface.render_allowed(owner, resumed),
            "retirement must invalidate the live generation",
        );
    }

    #[kani::proof]
    fn stale_owner_and_generation_are_rejected_without_transition() {
        let owner: u64 = kani::any();
        let other: u64 = kani::any();
        kani::assume(other != owner);
        let mut surface = SurfaceLifecycle::new(owner, 1, 1);
        let initial = surface.status(owner).unwrap();
        assert_eq!(
            surface.reconfigure(other, initial.generation, 2, 2),
            Err(SurfaceLifecycleError::WrongOwner)
        );
        assert_eq!(surface.status(owner).unwrap(), initial);
        assert_eq!(
            surface.suspend(owner, initial.generation + 1),
            Err(SurfaceLifecycleError::StaleGeneration)
        );
        assert_eq!(surface.status(owner).unwrap(), initial);
    }
}
