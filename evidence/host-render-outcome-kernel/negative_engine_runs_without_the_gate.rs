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



pub closed spec fn coordinator_surface(c:SurfaceCoordinator)->SurfaceLifecycle { c.surface }
pub closed spec fn coordinator_pending(c:SurfaceCoordinator)->bool { c.present_pending }
pub closed spec fn surface_admits_close(o:SurfaceLifecycle,caller:u64)->bool {
    caller==o.owner && !(o.state is Retired)
}
pub closed spec fn surface_admits_active(o:SurfaceLifecycle,caller:u64,generation:u64)->bool {
    caller==o.owner && generation==o.generation && o.state is Active && o.generation<u64::MAX
}
pub closed spec fn surface_admits_suspended(o:SurfaceLifecycle,caller:u64,generation:u64)->bool {
    caller==o.owner && generation==o.generation && o.state is Suspended && o.generation<u64::MAX
}
pub closed spec fn surface_admits_retire(o:SurfaceLifecycle,caller:u64,generation:u64)->bool {
    caller==o.owner && generation==o.generation && !(o.state is Retired)
}
// One link of any coordinator trace. Moving the lifecycle always drops a pending present; only an
// active lifecycle that stays put may gain one.
// 协调器任意轨迹的一环：生命周期迁移总是丢弃待呈现帧；只有原地不动的活动生命周期才能获得一帧。
pub closed spec fn surface_coordinator_marked(o:SurfaceCoordinator,n:SurfaceCoordinator)->bool {
    n.surface==o.surface && o.surface.state is Active && n.present_pending
}
pub closed spec fn surface_coordinator_step(o:SurfaceCoordinator,n:SurfaceCoordinator)->bool {
    n==o
    || (surface_step(o.surface,n.surface) && !n.present_pending)
    || surface_coordinator_marked(o,n)
}
pub closed spec fn surface_coordinator_trace(t:Seq<SurfaceCoordinator>)->bool {
    forall|k:int| 0<=k<t.len()-1 ==> #[trigger] surface_coordinator_step(t[k],t[k+1])
}
// The lifecycle underneath a coordinator trace is itself a lifecycle trace, so every lifecycle theorem carries over.
// 协调器轨迹之下的生命周期本身就是生命周期轨迹，生命周期的定理全部沿用。
proof fn surface_coordinator_trace_projects(t:Seq<SurfaceCoordinator>)
    requires surface_coordinator_trace(t),
    ensures surface_trace(t.map_values(|c:SurfaceCoordinator| c.surface)),
{
    reveal(surface_coordinator_trace); reveal(surface_coordinator_step); reveal(surface_coordinator_marked); reveal(surface_trace); reveal(surface_step);
    let s=t.map_values(|c:SurfaceCoordinator| c.surface);
    assert forall|k:int| 0<=k<s.len()-1 implies #[trigger] surface_step(s[k],s[k+1]) by {
        assert(surface_coordinator_step(t[k],t[k+1]));
        assert(s[k]==t[k].surface && s[k+1]==t[k+1].surface);
    }
}
// A pending present traces back to the step that rendered it, and the lifecycle has not moved
// since: the host can only present what the current generation rendered.
// 待呈现帧可追溯到渲染它的那一步，且此后生命周期没有迁移：宿主只能呈现当前代数渲染的内容。
proof fn surface_pending_comes_from_this_generation(t:Seq<SurfaceCoordinator>,j:int)->(i:int)
    requires surface_coordinator_trace(t), 0<=j<t.len(), !t[0].present_pending, t[j].present_pending,
    ensures 0<=i<j, surface_coordinator_marked(t[i],t[i+1]),
        forall|k:int| i<k<=j ==> (#[trigger] t[k]).surface==t[j].surface && t[k].present_pending,
    decreases j,
{
    reveal(surface_coordinator_trace); reveal(surface_coordinator_step);
    assert(j>0);
    let k=j-1;
    assert(surface_coordinator_step(t[k],t[k+1]));
    assert(k+1==j);
    if surface_coordinator_marked(t[k],t[j]) {
        reveal(surface_coordinator_marked);
        assert forall|m:int| k<m<=j implies (#[trigger] t[m]).surface==t[j].surface && t[m].present_pending by { assert(m==j); }
        k
    } else {
        assert(t[j]==t[k]);
        let i=surface_pending_comes_from_this_generation(t,k);
        assert forall|m:int| i<m<=j implies (#[trigger] t[m]).surface==t[j].surface && t[m].present_pending by {
            if m<j { assert(i<m<=k); }
        }
        i
    }
}
// A pending present only exists while the lifecycle is active.
// 待呈现帧只在生命周期活动时存在。
proof fn surface_pending_is_active(t:Seq<SurfaceCoordinator>,j:int)
    requires surface_coordinator_trace(t), 0<=j<t.len(), !t[0].present_pending, t[j].present_pending,
    ensures t[j].surface.state is Active,
{
    let i=surface_pending_comes_from_this_generation(t,j);
    reveal(surface_coordinator_marked);
    assert(t[i+1].surface==t[j].surface);
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
        self.state = SurfaceState::Retired;
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

/// Why a coordinated step did not advance. `Lifecycle` means the engine was never asked.
/// 协调步骤未推进的原因；`Lifecycle` 表示引擎根本未被调用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceStepError {
    Lifecycle(SurfaceLifecycleError),
    EngineRejected,
}

/// A lifecycle, the engine work that goes with each of its steps, and whether a texture frame
/// rendered at the current generation still waits for the host's present.
/// 生命周期、每一步配套的引擎工作，以及当前代数渲染的纹理帧是否仍在等待宿主呈现。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceCoordinator {
    surface: SurfaceLifecycle,
    present_pending: bool,
}

impl SurfaceCoordinator {
    pub fn new(owner: u64, width: u32, height: u32) -> (result: Self)
        ensures surface_new(coordinator_surface(result),owner,width,height), !coordinator_pending(result),
    { reveal(coordinator_surface); reveal(coordinator_pending);
        Self {
            surface: SurfaceLifecycle::new(owner, width, height),
            present_pending: false,
        }
    }

    pub fn status(&self,caller:u64)->(result:Option<SurfaceStatus>) ensures surface_read(coordinator_surface(*self),caller,result), { reveal(coordinator_surface);
        self.surface.status(caller)
    }

    pub fn render_allowed(&self,caller:u64,generation:u64)->(result:bool) ensures result==surface_live(coordinator_surface(*self),caller,generation), { reveal(coordinator_surface); reveal(surface_live); reveal(surface_gate_model);
        self.surface.render_allowed(caller, generation)
    }

    pub(crate) fn active_for_owner(&self,caller:u64)->(result:bool) ensures result==(caller==surface_gate_model(&coordinator_surface(*self)).0 && surface_gate_model(&coordinator_surface(*self)).1), { reveal(coordinator_surface);
        self.surface.active_for_owner(caller)
    }

    pub fn present_pending(&self)->(result:bool) ensures result==coordinator_pending(*self), { reveal(coordinator_pending);
        self.present_pending
    }

    /// Run `engine` only if the lifecycle admits the step, then commit the step. The step is taken
    /// on a copy first, so an admitted step cannot fail after the engine has done its work.
    /// 仅当生命周期准入时才运行 `engine`，随后提交。先在副本上迁移，因此已准入的步骤不会在引擎完成后失败。
    pub fn reconfigure(&mut self,caller:u64,expected_generation:u64,width:u32,height:u32,engine:impl FnOnce()->bool)->(result:Result<u64,SurfaceStepError>)
        requires surface_admits_active(coordinator_surface(*old(self)),caller,expected_generation) ==> engine.requires(()),
        ensures match result {
            Ok(value)=>{ let reconfigured=value; engine.ensures((),true) && surface_reconfigured(coordinator_surface(*old(self)),coordinator_surface(*final(self)),caller,expected_generation,width,height,Ok::<u64,SurfaceLifecycleError>(reconfigured)) && !coordinator_pending(*final(self)) },
            Err(SurfaceStepError::EngineRejected)=>engine.ensures((),false) && surface_admits_active(coordinator_surface(*old(self)),caller,expected_generation) && *final(self)==*old(self),
            Err(SurfaceStepError::Lifecycle(error))=>!(surface_admits_active(coordinator_surface(*old(self)),caller,expected_generation)) && surface_reconfigured(coordinator_surface(*old(self)),coordinator_surface(*old(self)),caller,expected_generation,width,height,Err::<u64,SurfaceLifecycleError>(error)) && *final(self)==*old(self),
        },
        surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_coordinator_step); reveal(surface_admits_active); reveal(surface_admits_suspended); reveal(surface_admits_retire);
      reveal(surface_reconfigured); reveal(surface_suspended); reveal(surface_resumed); reveal(surface_retired); reveal(surface_refusal); reveal(surface_advanced); reveal(surface_closed);
        let mut next = self.surface;
        let reconfigured = match next.reconfigure(caller, expected_generation, width, height) {
            Ok(generation) => generation,
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        };
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(reconfigured)
    }

    pub fn suspend(&mut self,caller:u64,expected_generation:u64,engine:impl FnOnce()->bool)->(result:Result<u64,SurfaceStepError>)
        requires surface_admits_active(coordinator_surface(*old(self)),caller,expected_generation) ==> engine.requires(()),
        ensures match result {
            Ok(value)=>{ let suspended=value; engine.ensures((),true) && surface_suspended(coordinator_surface(*old(self)),coordinator_surface(*final(self)),caller,expected_generation,Ok::<u64,SurfaceLifecycleError>(suspended)) && !coordinator_pending(*final(self)) },
            Err(SurfaceStepError::EngineRejected)=>engine.ensures((),false) && surface_admits_active(coordinator_surface(*old(self)),caller,expected_generation) && *final(self)==*old(self),
            Err(SurfaceStepError::Lifecycle(error))=>!(surface_admits_active(coordinator_surface(*old(self)),caller,expected_generation)) && surface_suspended(coordinator_surface(*old(self)),coordinator_surface(*old(self)),caller,expected_generation,Err::<u64,SurfaceLifecycleError>(error)) && *final(self)==*old(self),
        },
        surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_coordinator_step); reveal(surface_admits_active); reveal(surface_admits_suspended); reveal(surface_admits_retire);
      reveal(surface_reconfigured); reveal(surface_suspended); reveal(surface_resumed); reveal(surface_retired); reveal(surface_refusal); reveal(surface_advanced); reveal(surface_closed);
        let mut next = self.surface;
        let suspended = match next.suspend(caller, expected_generation) {
            Ok(generation) => generation,
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        };
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(suspended)
    }

    pub fn resume(&mut self,caller:u64,expected_generation:u64,width:u32,height:u32,engine:impl FnOnce()->bool)->(result:Result<u64,SurfaceStepError>)
        requires surface_admits_suspended(coordinator_surface(*old(self)),caller,expected_generation) ==> engine.requires(()),
        ensures match result {
            Ok(value)=>{ let resumed=value; engine.ensures((),true) && surface_resumed(coordinator_surface(*old(self)),coordinator_surface(*final(self)),caller,expected_generation,width,height,Ok::<u64,SurfaceLifecycleError>(resumed)) && !coordinator_pending(*final(self)) },
            Err(SurfaceStepError::EngineRejected)=>engine.ensures((),false) && surface_admits_suspended(coordinator_surface(*old(self)),caller,expected_generation) && *final(self)==*old(self),
            Err(SurfaceStepError::Lifecycle(error))=>!(surface_admits_suspended(coordinator_surface(*old(self)),caller,expected_generation)) && surface_resumed(coordinator_surface(*old(self)),coordinator_surface(*old(self)),caller,expected_generation,width,height,Err::<u64,SurfaceLifecycleError>(error)) && *final(self)==*old(self),
        },
        surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_coordinator_step); reveal(surface_admits_active); reveal(surface_admits_suspended); reveal(surface_admits_retire);
      reveal(surface_reconfigured); reveal(surface_suspended); reveal(surface_resumed); reveal(surface_retired); reveal(surface_refusal); reveal(surface_advanced); reveal(surface_closed);
        let mut next = self.surface;
        let resumed = match next.resume(caller, expected_generation, width, height) {
            Ok(generation) => generation,
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        };
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(resumed)
    }

    pub fn retire(&mut self,caller:u64,expected_generation:u64,engine:impl FnOnce()->bool)->(result:Result<(),SurfaceStepError>)
        requires surface_admits_retire(coordinator_surface(*old(self)),caller,expected_generation) ==> engine.requires(()),
        ensures match result {
            Ok(value)=>{ let retired=value; engine.ensures((),true) && surface_retired(coordinator_surface(*old(self)),coordinator_surface(*final(self)),caller,expected_generation,Ok::<(),SurfaceLifecycleError>(retired)) && !coordinator_pending(*final(self)) },
            Err(SurfaceStepError::EngineRejected)=>engine.ensures((),false) && surface_admits_retire(coordinator_surface(*old(self)),caller,expected_generation) && *final(self)==*old(self),
            Err(SurfaceStepError::Lifecycle(error))=>!(surface_admits_retire(coordinator_surface(*old(self)),caller,expected_generation)) && surface_retired(coordinator_surface(*old(self)),coordinator_surface(*old(self)),caller,expected_generation,Err::<(),SurfaceLifecycleError>(error)) && *final(self)==*old(self),
        },
        surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_coordinator_step); reveal(surface_admits_active); reveal(surface_admits_suspended); reveal(surface_admits_retire);
      reveal(surface_reconfigured); reveal(surface_suspended); reveal(surface_resumed); reveal(surface_retired); reveal(surface_refusal); reveal(surface_advanced); reveal(surface_closed);
        let mut next = self.surface;
        match next.retire(caller, expected_generation) {
            Ok(()) => {}
            Err(error) => return Err(SurfaceStepError::Lifecycle(error)),
        }
        if !engine() {
            return Err(SurfaceStepError::EngineRejected);
        }
        self.adopt(next);
        Ok(())
    }

    /// Retirement without a generation, for cleanup paths that hold no current surface.
    /// 不带代数的退休，用于不持有当前 Surface 的清理路径。
    pub fn close(&mut self,caller:u64,engine:impl FnOnce()->bool)->(result:bool)
        requires surface_admits_close(coordinator_surface(*old(self)),caller) ==> engine.requires(()),
        ensures result ==> engine.ensures((),true) && surface_retired_current(coordinator_surface(*old(self)),coordinator_surface(*final(self)),caller,true) && !coordinator_pending(*final(self)),
            !result ==> *final(self)==*old(self) && (surface_admits_close(coordinator_surface(*old(self)),caller) ==> engine.ensures((),false)),
            surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_admits_close); reveal(surface_coordinator_step); reveal(surface_retired_current); reveal(surface_closed);
        let mut next = self.surface;
        if !next.retire_current(caller) {
            return false;
        }
        if !engine() {
            return false;
        }
        self.adopt(next);
        true
    }

    /// The pixels waiting for a present were superseded; the host must not present them.
    /// 等待呈现的像素已被取代，宿主不得再呈现。
    pub fn frame_superseded(&mut self)
        ensures coordinator_surface(*final(self))==coordinator_surface(*old(self)), !coordinator_pending(*final(self)), surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_coordinator_step); reveal(surface_step);
        self.present_pending = false;
    }

    /// Record a texture frame rendered at exactly the live generation.
    /// 记录恰在当前活动代数渲染的纹理帧。
    pub fn texture_rendered(&mut self,caller:u64,generation:u64)->(result:bool)
        ensures result==surface_live(coordinator_surface(*old(self)),caller,generation),
            coordinator_surface(*final(self))==coordinator_surface(*old(self)),
            coordinator_pending(*final(self))==(result || coordinator_pending(*old(self))),
            surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_coordinator_step); reveal(surface_coordinator_marked); reveal(surface_live); reveal(surface_gate_model);
        if !self.surface.render_allowed(caller, generation) {
            return false;
        }
        self.present_pending = true;
        true
    }

    /// Acknowledge the host's present of the pending frame at exactly the live generation.
    /// 确认宿主恰在当前活动代数呈现了待呈现帧。
    pub fn presented(&mut self,caller:u64,generation:u64)->(result:bool)
        ensures result==(surface_live(coordinator_surface(*old(self)),caller,generation) && coordinator_pending(*old(self))),
            coordinator_surface(*final(self))==coordinator_surface(*old(self)),
            coordinator_pending(*final(self))==(coordinator_pending(*old(self)) && !result),
            surface_coordinator_step(*old(self),*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending); reveal(surface_coordinator_step); reveal(surface_step); reveal(surface_live); reveal(surface_gate_model);
        if !self.surface.render_allowed(caller, generation) || !self.present_pending {
            return false;
        }
        self.present_pending = false;
        true
    }

    fn adopt(&mut self,next:SurfaceLifecycle)
        ensures coordinator_surface(*final(self))==next, !coordinator_pending(*final(self)),
    { reveal(coordinator_surface); reveal(coordinator_pending);
        self.surface = next;
        self.present_pending = false;
    }
}

/// Where a successful render leaves its pixels.
/// 渲染成功后像素所在的位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderedPixels {
    /// Already in the host's hands (a copied buffer, a backend that presented). Nothing to confirm.
    /// 已交到宿主手中（已复制的缓冲区、已呈现的后端），无需确认。
    Delivered,
    /// In a host texture that the host must still present and acknowledge.
    /// 在宿主纹理中，宿主仍须呈现并确认。
    AwaitingPresent,
}

impl SurfaceCoordinator {
    /// Run `engine` only at the exact live generation. `None` means it was never asked. A failed
    /// render may have touched the target, so it leaves no frame the host could still acknowledge.
    /// 仅在恰为当前活动代数时运行 `engine`；`None` 表示未调用。失败的渲染可能已写过目标，因此不留下宿主还能确认的帧。
    pub fn render<T,E>(&mut self,caller:u64,generation:u64,pixels:RenderedPixels,engine:impl FnOnce()->Result<T,E>)->(result:Option<Result<T,E>>)
        requires surface_live(coordinator_surface(*old(self)),caller,generation) ==> engine.requires(()),
        ensures coordinator_surface(*final(self))==coordinator_surface(*old(self)),
            surface_coordinator_step(*old(self),*final(self)),
            match result {
                None => !surface_live(coordinator_surface(*old(self)),caller,generation) && *final(self)==*old(self),
                Some(outcome) => surface_live(coordinator_surface(*old(self)),caller,generation)
                    && engine.ensures((),outcome)
                    && coordinator_pending(*final(self))==(outcome is Ok && pixels is AwaitingPresent),
            },
    {
                let result = engine();
        match &result {
            Ok(_) => match pixels {
                RenderedPixels::Delivered => self.frame_superseded(),
                RenderedPixels::AwaitingPresent => {
                    self.texture_rendered(caller, generation);
                }
            },
            Err(_) => self.frame_superseded(),
        }
        Some(result)
    }
}

}
