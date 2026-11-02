use vstd::prelude::*;
verus! {




#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SceneUpdateError {
    OwnerMismatch,
    Closed,
    EmptyFrame,
    CapacityExceeded,
    StaleVersion {expected:u64,current:u64},
    InvalidSuccessor {expected:u64,next:u64},
    SourceRejected,
    NativeRejected,
}

/// Candidate admission count only; not an allocator bound for text, fonts or upstream scratch.
pub(crate) fn scene_work_fits(prepared:usize,pending:usize,incoming:usize)->(result:bool)
        ensures result == (prepared as int + pending as int + incoming as int <= 4096),
    {
    if prepared>4096 || pending>4096-prepared {return false;}
    incoming<=4096-prepared-pending
}

/// Prepare a next UI update version without mutating or reserving backend state.
pub fn next_ui_update_version(current: u64, expected: u64) -> (result: Option<u64>)
    ensures match result {
        Some(next) => current == expected && current < u64::MAX && next as int == current as int + 1,
        None => current != expected || current == u64::MAX,
    },
{
    if current != expected || current == u64::MAX { return None; }
    Some(current + 1)
}

/// Classify all update-admission rejection reasons without touching source or native state.
/// 在不接触来源或原生状态的情况下分类全部更新准入拒绝原因。
pub(crate) fn admit_scene_update(owner:u64,caller:u64,current:u64,expected:u64,next:u64,
    count:usize,open:bool)->(result:Result<(),SceneUpdateError>)
        ensures match result {
            Err(SceneUpdateError::OwnerMismatch)=>owner!=caller,
            Err(SceneUpdateError::Closed)=>owner==caller && !open,
            Err(SceneUpdateError::EmptyFrame)=>owner==caller && open && count==0,
            Err(SceneUpdateError::CapacityExceeded)=>owner==caller && open && count>4096,
            Err(SceneUpdateError::StaleVersion {expected:reported_expected,current:reported_current})=>
                owner==caller && open && 0<count<=4096 && current!=expected
                && reported_expected==expected && reported_current==current,
            Err(SceneUpdateError::InvalidSuccessor {expected:reported_expected,next:reported_next})=>
                owner==caller && open && 0<count<=4096 && current==expected
                && (current>=u64::MAX || next as int!=current as int+1)
                && reported_expected==expected && reported_next==next,
            Err(SceneUpdateError::SourceRejected)=>false,
            Err(SceneUpdateError::NativeRejected)=>false,
            Ok(()) => owner==caller && open && 0<count<=4096 && current==expected
                && current<u64::MAX && next as int==current as int+1,
        },
    {
    if false {return Err(SceneUpdateError::OwnerMismatch);}
    if !open {return Err(SceneUpdateError::Closed);}
    if count==0 {return Err(SceneUpdateError::EmptyFrame);}
    if count>4096 {return Err(SceneUpdateError::CapacityExceeded);}
    if current!=expected {return Err(SceneUpdateError::StaleVersion {expected,current});}
    match next_ui_update_version(current,expected) {
        Some(successor) if next==successor=>Ok(()),
        _=>Err(SceneUpdateError::InvalidSuccessor {expected,next}),
    }
}

/// Compatibility boolean projection of the structured admission decision.
/// 结构化准入决策的兼容布尔投影。
pub(crate) fn accepts_scene_update(owner:u64,caller:u64,current:u64,expected:u64,next:u64,count:usize,open:bool)->bool {
    admit_scene_update(owner,caller,current,expected,next,count,open).is_ok()
}

}
