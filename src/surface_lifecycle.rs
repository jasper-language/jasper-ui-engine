//! Host-neutral presentation-surface generations and lifecycle.
//! 宿主中立的呈现 Surface 代数与生命周期。
//!
//! The host owns the actual native surface, texture, device and present call. This state
//! machine only rejects stale callbacks after resize, suspend, resume or retirement.
//! 原生 Surface、纹理、设备与 present 都归宿主；本状态机只拒绝尺寸变化、暂停、恢复或
//! 退休后的迟到回调。

/// Logical presentation-surface state. A suspended surface may be resumed; a retired one
/// never can.
/// 逻辑呈现 Surface 状态。暂停状态可恢复；退休状态永不恢复。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceState {
    Active,
    Suspended,
    Retired,
}

/// Current extent and generation visible to the owning host.
/// 所属宿主可见的当前尺寸与代数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceStatus {
    pub state: SurfaceState,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
}

/// Why a lifecycle transition was rejected. Rejections leave the transition state unchanged.
/// 生命周期迁移被拒原因；拒绝时迁移状态保持不变。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceLifecycleError {
    WrongOwner,
    WrongState,
    StaleGeneration,
    GenerationExhausted,
}

impl std::fmt::Display for SurfaceLifecycleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongOwner => write!(formatter, "surface callback owner does not match"),
            Self::WrongState => write!(formatter, "surface lifecycle state rejects the transition"),
            Self::StaleGeneration => write!(formatter, "surface generation is stale"),
            Self::GenerationExhausted => write!(formatter, "surface generation is exhausted"),
        }
    }
}

impl std::error::Error for SurfaceLifecycleError {}

/// One host-owned surface generation. It contains no native handle and performs no I/O.
/// 一个宿主持有的 Surface 代数；不含原生句柄，也不执行 I/O。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceLifecycle {
    owner: u64,
    state: SurfaceState,
    generation: u64,
    width: u32,
    height: u32,
}

impl SurfaceLifecycle {
    /// Initial active host surface generation.
    /// 初始活动宿主 Surface 代数。
    pub const fn new(owner: u64, width: u32, height: u32) -> Self {
        Self {
            owner,
            state: SurfaceState::Active,
            generation: 1,
            width,
            height,
        }
    }

    pub fn status(&self, caller: u64) -> Option<SurfaceStatus> {
        if caller != self.owner {
            return None;
        }
        Some(SurfaceStatus {
            state: self.state,
            generation: self.generation,
            width: self.width,
            height: self.height,
        })
    }

    /// True only for the exact live generation of the owning host.
    /// 仅当宿主身份与当前活动代数精确匹配时为真。
    pub fn render_allowed(&self, caller: u64, generation: u64) -> bool {
        caller == self.owner && self.state == SurfaceState::Active && self.generation == generation
    }

    /// True while the owning host may submit work without carrying an epoch. Public rendering
    /// entry points use this compatibility view; generation-checked entry points are preferred.
    /// 所属宿主无需携带代数即可提交工作时的兼容视图；优先使用携带代数的渲染入口。
    pub(crate) fn active_for_owner(&self, caller: u64) -> bool {
        caller == self.owner && self.state == SurfaceState::Active
    }

    /// Replace the host extent. The old generation immediately becomes stale.
    /// 替换宿主尺寸；旧代数立即失效。
    pub fn reconfigure(
        &mut self,
        caller: u64,
        expected_generation: u64,
        width: u32,
        height: u32,
    ) -> Result<u64, SurfaceLifecycleError> {
        self.require_active(caller, expected_generation)?;
        let next = self.next_generation()?;
        self.generation = next;
        self.width = width;
        self.height = height;
        Ok(next)
    }

    /// Release host presentation resources. A stale callback cannot suspend a successor.
    /// 释放宿主呈现资源；迟到回调不能暂停后继 Surface。
    pub fn suspend(
        &mut self,
        caller: u64,
        expected_generation: u64,
    ) -> Result<u64, SurfaceLifecycleError> {
        self.require_active(caller, expected_generation)?;
        let next = self.next_generation()?;
        self.generation = next;
        self.state = SurfaceState::Suspended;
        Ok(next)
    }

    /// Recreate host presentation resources for a new generation.
    /// 为新代数重建宿主呈现资源。
    pub fn resume(
        &mut self,
        caller: u64,
        expected_generation: u64,
        width: u32,
        height: u32,
    ) -> Result<u64, SurfaceLifecycleError> {
        self.require(caller, expected_generation)?;
        if self.state != SurfaceState::Suspended {
            return Err(SurfaceLifecycleError::WrongState);
        }
        let next = self.next_generation()?;
        self.generation = next;
        self.width = width;
        self.height = height;
        self.state = SurfaceState::Active;
        Ok(next)
    }

    /// Permanently retire this host surface. Repeated retirement is rejected.
    /// 永久退休该宿主 Surface；重复退休被拒绝。
    pub fn retire(
        &mut self,
        caller: u64,
        expected_generation: u64,
    ) -> Result<(), SurfaceLifecycleError> {
        self.require(caller, expected_generation)?;
        if self.state == SurfaceState::Retired {
            return Err(SurfaceLifecycleError::WrongState);
        }
        if self.generation < u64::MAX {
            self.generation += 1;
        }
        self.state = SurfaceState::Retired;
        Ok(())
    }

    /// Public owner/state retirement compatibility path. It has no generation guard.
    /// 公开 owner/state 兼容退休路径；不携带代数保护。
    pub(crate) fn retire_current(&mut self, caller: u64) -> bool {
        if caller != self.owner || self.state == SurfaceState::Retired {
            return false;
        }
        if self.generation < u64::MAX {
            self.generation += 1;
        }
        self.state = SurfaceState::Retired;
        true
    }

    fn require_active(
        &self,
        caller: u64,
        expected_generation: u64,
    ) -> Result<(), SurfaceLifecycleError> {
        self.require(caller, expected_generation)?;
        if self.state != SurfaceState::Active {
            return Err(SurfaceLifecycleError::WrongState);
        }
        Ok(())
    }

    fn require(&self, caller: u64, expected_generation: u64) -> Result<(), SurfaceLifecycleError> {
        if caller != self.owner {
            return Err(SurfaceLifecycleError::WrongOwner);
        }
        if expected_generation != self.generation {
            return Err(SurfaceLifecycleError::StaleGeneration);
        }
        Ok(())
    }

    fn next_generation(&self) -> Result<u64, SurfaceLifecycleError> {
        self.generation
            .checked_add(1)
            .ok_or(SurfaceLifecycleError::GenerationExhausted)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn transition_generations_invalidate_old_host_resources() {
        let mut surface = SurfaceLifecycle::new(7, 320, 200);
        assert!(surface.render_allowed(7, 1));
        assert_eq!(surface.reconfigure(7, 1, 640, 360), Ok(2));
        assert!(!surface.render_allowed(7, 1));
        assert!(surface.render_allowed(7, 2));
        assert_eq!(
            surface.suspend(7, 1),
            Err(SurfaceLifecycleError::StaleGeneration)
        );
        assert_eq!(surface.suspend(7, 2), Ok(3));
        assert!(!surface.render_allowed(7, 3));
        assert_eq!(
            surface.resume(7, 2, 800, 600),
            Err(SurfaceLifecycleError::StaleGeneration)
        );
        assert_eq!(surface.resume(7, 3, 800, 600), Ok(4));
        assert!(surface.render_allowed(7, 4));
        assert_eq!(surface.retire(7, 4), Ok(()));
        let retired_generation = surface.status(7).unwrap().generation;
        assert_eq!(
            surface.resume(7, retired_generation, 1, 1),
            Err(SurfaceLifecycleError::WrongState)
        );
        assert_eq!(
            surface.retire(7, retired_generation),
            Err(SurfaceLifecycleError::WrongState)
        );
    }

    #[test]
    fn foreign_callbacks_do_not_change_state() {
        let mut surface = SurfaceLifecycle::new(7, 320, 200);
        let initial = surface.status(7);
        assert_eq!(
            surface.reconfigure(8, 1, 1, 1),
            Err(SurfaceLifecycleError::WrongOwner)
        );
        assert_eq!(surface.status(7), initial);
        assert_eq!(surface.status(8), None);
    }

    proptest! {
        #[test]
        fn successful_cycles_stay_active_and_monotone(
            owner in any::<u64>(),
            first_w in any::<u32>(),
            first_h in any::<u32>(),
            next_w in any::<u32>(),
            next_h in any::<u32>(),
        ) {
            let mut surface = SurfaceLifecycle::new(owner, first_w, first_h);
            let reconfigured = surface.reconfigure(owner, 1, next_w, next_h).unwrap();
            prop_assert!(reconfigured > 1);
            let suspended = surface.suspend(owner, reconfigured).unwrap();
            prop_assert!(suspended > reconfigured);
            let resumed = surface.resume(owner, suspended, first_w, first_h).unwrap();
            prop_assert!(resumed > suspended);
            prop_assert!(surface.render_allowed(owner, resumed));
            prop_assert_eq!(surface.status(owner).unwrap().width, first_w);
            prop_assert_eq!(surface.status(owner).unwrap().height, first_h);
        }
    }
}
