use vstd::prelude::*;
verus! {




pub closed spec fn surface_gate_model(s: &SurfaceLifecycle) -> (u64, bool, u64) {
    (s.owner, s.state is Active, s.generation)
}
// The checks every generation-carrying transition starts with, in the order they are made.
// 每个携带代数的迁移一开始的检查，按先后写出。
pub closed spec fn surface_refusal(o:SurfaceLifecycle,caller:u64,generation:u64)->Option<SurfaceLifecycleError> {
    if caller!=o.owner { Some(SurfaceLifecycleError::WrongOwner) }
    else if generation!=o.generation { Some(SurfaceLifecycleError::StaleGeneration) }
    else { None }
}
pub closed spec fn surface_advanced(o:SurfaceLifecycle,n:SurfaceLifecycle,state:SurfaceState,width:u32,height:u32)->bool {
    o.generation<u64::MAX && n==(SurfaceLifecycle{owner:o.owner,state:state,generation:(o.generation+1) as u64,width:width,height:height})
}
pub closed spec fn surface_reconfigured(o:SurfaceLifecycle,n:SurfaceLifecycle,caller:u64,generation:u64,width:u32,height:u32,r:Result<u64,SurfaceLifecycleError>)->bool {
    match surface_refusal(o,caller,generation) {
        Some(e)=>r==Err::<u64,SurfaceLifecycleError>(e) && n==o,
        None=>if !(o.state is Active) { r==Err::<u64,SurfaceLifecycleError>(SurfaceLifecycleError::WrongState) && n==o }
            else if o.generation==u64::MAX { r==Err::<u64,SurfaceLifecycleError>(SurfaceLifecycleError::GenerationExhausted) && n==o }
            else { surface_advanced(o,n,SurfaceState::Active,width,height) && r==Ok::<u64,SurfaceLifecycleError>(n.generation) },
    }
}
pub closed spec fn surface_suspended(o:SurfaceLifecycle,n:SurfaceLifecycle,caller:u64,generation:u64,r:Result<u64,SurfaceLifecycleError>)->bool {
    match surface_refusal(o,caller,generation) {
        Some(e)=>r==Err::<u64,SurfaceLifecycleError>(e) && n==o,
        None=>if !(o.state is Active) { r==Err::<u64,SurfaceLifecycleError>(SurfaceLifecycleError::WrongState) && n==o }
            else if o.generation==u64::MAX { r==Err::<u64,SurfaceLifecycleError>(SurfaceLifecycleError::GenerationExhausted) && n==o }
            else { surface_advanced(o,n,SurfaceState::Suspended,o.width,o.height) && r==Ok::<u64,SurfaceLifecycleError>(n.generation) },
    }
}
pub closed spec fn surface_resumed(o:SurfaceLifecycle,n:SurfaceLifecycle,caller:u64,generation:u64,width:u32,height:u32,r:Result<u64,SurfaceLifecycleError>)->bool {
    match surface_refusal(o,caller,generation) {
        Some(e)=>r==Err::<u64,SurfaceLifecycleError>(e) && n==o,
        None=>if !(o.state is Suspended) { r==Err::<u64,SurfaceLifecycleError>(SurfaceLifecycleError::WrongState) && n==o }
            else if o.generation==u64::MAX { r==Err::<u64,SurfaceLifecycleError>(SurfaceLifecycleError::GenerationExhausted) && n==o }
            else { surface_advanced(o,n,SurfaceState::Active,width,height) && r==Ok::<u64,SurfaceLifecycleError>(n.generation) },
    }
}
// Retirement never fails on exhaustion: the generation saturates and the state alone closes it.
// 退休不因耗尽失败：代数饱和，只靠状态关闭。
pub closed spec fn surface_closed(o:SurfaceLifecycle,n:SurfaceLifecycle)->bool {
    !(o.state is Retired) && n==(SurfaceLifecycle{owner:o.owner,state:SurfaceState::Retired,
        generation:if o.generation<u64::MAX { (o.generation+1) as u64 } else { o.generation },width:o.width,height:o.height})
}
pub closed spec fn surface_retired(o:SurfaceLifecycle,n:SurfaceLifecycle,caller:u64,generation:u64,r:Result<(),SurfaceLifecycleError>)->bool {
    match surface_refusal(o,caller,generation) {
        Some(e)=>r==Err::<(),SurfaceLifecycleError>(e) && n==o,
        None=>if o.state is Retired { r==Err::<(),SurfaceLifecycleError>(SurfaceLifecycleError::WrongState) && n==o }
            else { surface_closed(o,n) && r is Ok },
    }
}
pub closed spec fn surface_retired_current(o:SurfaceLifecycle,n:SurfaceLifecycle,caller:u64,r:bool)->bool {
    if caller!=o.owner || o.state is Retired { !r && n==o } else { r && surface_closed(o,n) }
}
pub closed spec fn surface_new(n:SurfaceLifecycle,owner:u64,width:u32,height:u32)->bool {
    n==(SurfaceLifecycle{owner:owner,state:SurfaceState::Active,generation:1,width:width,height:height})
}
pub closed spec fn surface_read(o:SurfaceLifecycle,caller:u64,r:Option<SurfaceStatus>)->bool {
    if caller!=o.owner { r is None } else { r==Some(SurfaceStatus{state:o.state,generation:o.generation,width:o.width,height:o.height}) }
}
// One link of any trace: nothing changed, or one successful transition.
// 任意轨迹的一环：没有变化，或一次成功迁移。
pub closed spec fn surface_step(o:SurfaceLifecycle,n:SurfaceLifecycle)->bool {
    let moved = o.generation<u64::MAX && n.owner==o.owner && n.generation==o.generation+1;
    n==o
    || (o.state is Active && moved && n.state is Active)
    || (o.state is Active && moved && n.state is Suspended && n.width==o.width && n.height==o.height)
    || (o.state is Suspended && moved && n.state is Active)
    || surface_closed(o,n)
}
pub closed spec fn surface_trace(t:Seq<SurfaceLifecycle>)->bool {
    forall|k:int| 0<=k<t.len()-1 ==> #[trigger] surface_step(t[k],t[k+1])
}
pub closed spec fn surface_live(s:SurfaceLifecycle,caller:u64,generation:u64)->bool {
    caller==s.owner && s.state is Active && generation==s.generation
}
// Generations never fall, the owner never changes, and leaving a live state cannot be undone.
// 代数不回退，归属不变，离开活动状态后不能撤销。
proof fn surface_trace_holds(t:Seq<SurfaceLifecycle>,i:int,j:int)
    requires surface_trace(t), 0<=i<=j<t.len(),
    ensures t[j].owner==t[i].owner, t[j].generation>=t[i].generation,
        t[i].state is Retired ==> t[j]==t[i],
        t[j]==t[i] || t[j].generation>t[i].generation || t[j].state is Retired,
    decreases j-i,
{
    reveal(surface_trace); reveal(surface_step); reveal(surface_advanced); reveal(surface_closed);
    if i<j {
        surface_trace_holds(t,i,j-1);
        let k=j-1;
        assert(surface_step(t[k],t[k+1]));
        assert(k+1==j);
    }
}
// A generation names one state: two moments where it may render are the same state, extent included.
// 一个代数只对应一个状态：它可渲染的两个时刻是同一状态，尺寸也相同。
proof fn surface_generation_names_one_state(t:Seq<SurfaceLifecycle>,i:int,j:int,caller:u64,generation:u64)
    requires surface_trace(t), 0<=i<=j<t.len(), surface_live(t[i],caller,generation), surface_live(t[j],caller,generation),
    ensures t[i]==t[j],
{
    reveal(surface_live);
    surface_trace_holds(t,i,j);
}
// Once retired, every later state is that state, so no generation renders again.
// 一旦退休，此后都是同一状态，任何代数都不再可渲染。
proof fn surface_retired_is_final(t:Seq<SurfaceLifecycle>,i:int,j:int,caller:u64,generation:u64)
    requires surface_trace(t), 0<=i<=j<t.len(), t[i].state is Retired,
    ensures t[j]==t[i], !surface_live(t[j],caller,generation),
{
    reveal(surface_live);
    surface_trace_holds(t,i,j);
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

impl SurfaceLifecycle {
    /// Initial active host surface generation.
    /// 初始活动宿主 Surface 代数。
    pub const fn new(owner: u64, width: u32, height: u32) -> (result: Self)
        ensures surface_new(result,owner,width,height),
    { reveal(surface_new);
        Self {
            owner,
            state: SurfaceState::Active,
            generation: 1,
            width,
            height,
        }
    }

    pub fn status(&self, caller: u64) -> (result: Option<SurfaceStatus>)
        ensures surface_read(*self,caller,result),
    { reveal(surface_read);
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

    /// Replace the host extent. The old generation immediately becomes stale.
    /// 替换宿主尺寸；旧代数立即失效。
    pub fn reconfigure(&mut self,caller:u64,expected_generation:u64,width:u32,height:u32)->(result:Result<u64,SurfaceLifecycleError>)
        ensures surface_reconfigured(*old(self),*final(self),caller,expected_generation,width,height,result),
            surface_step(*old(self),*final(self)),
    { reveal(surface_reconfigured); reveal(surface_refusal); reveal(surface_advanced); reveal(surface_step);
        self.require_active(caller, expected_generation)?;
        let next = self.next_generation()?;
        self.generation = next;
        self.width = width;
        self.height = height;
        Ok(next)
    }

    /// Release host presentation resources. A stale callback cannot suspend a successor.
    /// 释放宿主呈现资源；迟到回调不能暂停后继 Surface。
    pub fn suspend(&mut self,caller:u64,expected_generation:u64)->(result:Result<u64,SurfaceLifecycleError>)
        ensures surface_suspended(*old(self),*final(self),caller,expected_generation,result),
            surface_step(*old(self),*final(self)),
    { reveal(surface_suspended); reveal(surface_refusal); reveal(surface_advanced); reveal(surface_step);
        self.require_active(caller, expected_generation)?;
        let next = self.next_generation()?;
        self.generation = next;
        self.state = SurfaceState::Suspended;
        Ok(next)
    }

    /// Recreate host presentation resources for a new generation.
    /// 为新代数重建宿主呈现资源。
    pub fn resume(&mut self,caller:u64,expected_generation:u64,width:u32,height:u32)->(result:Result<u64,SurfaceLifecycleError>)
        ensures surface_resumed(*old(self),*final(self),caller,expected_generation,width,height,result),
            surface_step(*old(self),*final(self)),
    { reveal(surface_resumed); reveal(surface_refusal); reveal(surface_advanced); reveal(surface_step);
        self.require(caller, expected_generation)?;
        if !matches!(self.state, SurfaceState::Suspended) {
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
    pub fn retire(&mut self,caller:u64,expected_generation:u64)->(result:Result<(),SurfaceLifecycleError>)
        ensures surface_retired(*old(self),*final(self),caller,expected_generation,result),
            surface_step(*old(self),*final(self)),
    { reveal(surface_retired); reveal(surface_refusal); reveal(surface_closed); reveal(surface_step);
        self.require(caller, expected_generation)?;
        if matches!(self.state, SurfaceState::Retired) {
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
    pub(crate) fn retire_current(&mut self,caller:u64)->(result:bool)
        ensures surface_retired_current(*old(self),*final(self),caller,result),
            surface_step(*old(self),*final(self)),
    { reveal(surface_retired_current); reveal(surface_closed); reveal(surface_step);
        if caller != self.owner || matches!(self.state, SurfaceState::Retired) {
            return false;
        }
        if self.generation < u64::MAX {
            self.generation += 1;
        }
        true
    }

    fn require_active(&self,caller:u64,expected_generation:u64)->(result:Result<(),SurfaceLifecycleError>)
        ensures match surface_refusal(*self,caller,expected_generation) {
            Some(e)=>result==Err::<(),SurfaceLifecycleError>(e),
            None=>if self.state is Active { result is Ok } else { result==Err::<(),SurfaceLifecycleError>(SurfaceLifecycleError::WrongState) },
        },
    { reveal(surface_refusal);
        self.require(caller, expected_generation)?;
        if !matches!(self.state, SurfaceState::Active) {
            return Err(SurfaceLifecycleError::WrongState);
        }
        Ok(())
    }

    fn require(&self,caller:u64,expected_generation:u64)->(result:Result<(),SurfaceLifecycleError>)
        ensures match surface_refusal(*self,caller,expected_generation) {
            Some(e)=>result==Err::<(),SurfaceLifecycleError>(e),
            None=>result is Ok,
        },
    { reveal(surface_refusal);
        if caller != self.owner {
            return Err(SurfaceLifecycleError::WrongOwner);
        }
        if expected_generation != self.generation {
            return Err(SurfaceLifecycleError::StaleGeneration);
        }
        Ok(())
    }

    fn next_generation(&self)->(result:Result<u64,SurfaceLifecycleError>)
        ensures if self.generation<u64::MAX { result==Ok::<u64,SurfaceLifecycleError>((self.generation+1) as u64) }
            else { result==Err::<u64,SurfaceLifecycleError>(SurfaceLifecycleError::GenerationExhausted) },
    {
        self.generation
            .checked_add(1)
            .ok_or(SurfaceLifecycleError::GenerationExhausted)
    }
}

}
