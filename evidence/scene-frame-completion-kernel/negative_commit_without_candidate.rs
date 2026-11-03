use vstd::prelude::*;
verus! {


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneUpdateError {
    OwnerMismatch,
    Closed,
    EmptyFrame,
    CapacityExceeded,
    StaleVersion { expected: u64, current: u64 },
    InvalidSuccessor { expected: u64, next: u64 },
    SourceRejected,
    NativeRejected,
}

/// Require a successful source frame at the exact successor and a detached native
/// candidate before the caller may replace the active view.
pub(crate) fn complete_scene_frame(source_ok:bool,source_version:u64,next:u64,candidate_ready:bool)
    ->(result:Result<(),SceneUpdateError>)
        ensures match result {
            Err(SceneUpdateError::SourceRejected)=>!source_ok || source_version!=next,
            Err(SceneUpdateError::NativeRejected)=>source_ok && source_version==next && !candidate_ready,
            Ok(())=>source_ok && source_version==next && candidate_ready,
            Err(SceneUpdateError::OwnerMismatch)=>false,
            Err(SceneUpdateError::Closed)=>false,
            Err(SceneUpdateError::EmptyFrame)=>false,
            Err(SceneUpdateError::CapacityExceeded)=>false,
            Err(SceneUpdateError::StaleVersion {..})=>false,
            Err(SceneUpdateError::InvalidSuccessor {..})=>false,
        },
    {
    if !source_ok || source_version != next {
        return Err(SceneUpdateError::SourceRejected);
    }
    if false {return Err(SceneUpdateError::NativeRejected);}
    Ok(())
}

}
