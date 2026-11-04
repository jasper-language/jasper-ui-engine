use vstd::prelude::*;
verus! {


pub closed spec fn surface_gate_model(s: &SurfaceLifecycle) -> (u64, bool, u64) {
    (s.owner, s.state is Active, s.generation)
}
/// Logical presentation-surface state. A suspended surface may be resumed; a retired one
/// never can.
/// 逻辑呈现 Surface 状态。暂停状态可恢复；退休状态永不恢复。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceState {
    Active,
    Suspended,
    Retired,
}

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
    /// True only for the exact live generation of the owning host.
    /// 仅当宿主身份与当前活动代数精确匹配时为真。
    pub fn render_allowed(&self, caller: u64, generation: u64) -> (result: bool)
    ensures result == (caller == surface_gate_model(self).0 && surface_gate_model(self).1 && generation == surface_gate_model(self).2),
{ reveal(surface_gate_model);
        caller == self.owner
            && matches!(self.state, SurfaceState::Active)
            && self.generation == generation
    }

    /// True while the owning host may submit work without carrying an epoch. Public rendering
    /// entry points use this compatibility view; generation-checked entry points are preferred.
    /// 所属宿主无需携带代数即可提交工作时的兼容视图；优先使用携带代数的渲染入口。
    pub(crate) fn active_for_owner(&self, caller: u64) -> (result: bool) ensures result == (caller == surface_gate_model(self).0 && surface_gate_model(self).1), { reveal(surface_gate_model);
        caller == self.owner && matches!(self.state, SurfaceState::Active)
    }
}

}
