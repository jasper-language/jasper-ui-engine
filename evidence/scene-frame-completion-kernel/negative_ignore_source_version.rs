use vstd::prelude::*;
verus! {


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneUpdateError {
    OwnerMismatch,
    Closed,
    EmptyFrame,
    StaleVersion {
        expected: u64,
        current: u64,
    },
    InvalidSuccessor {
        expected: u64,
        next: u64,
    },
    SourceRejected,
    NativeRejected,
    /// The updated scene would hold `required` nodes; the host quota allows `limit`.
    /// 更新后的场景需要 `required` 个节点，宿主额度只允许 `limit` 个。
    NodeQuotaExceeded {
        limit: usize,
        required: usize,
    },
    /// The updated scene would hold `required` bytes of text and semantic strings; the host
    /// quota allows `limit`.
    /// 更新后的场景需要 `required` 字节文字与语义字符串，宿主额度只允许 `limit` 字节。
    TextQuotaExceeded {
        limit: usize,
        required: usize,
    },
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
            Err(SceneUpdateError::NodeQuotaExceeded {..})=>false,
            Err(SceneUpdateError::TextQuotaExceeded {..})=>false,
            Err(SceneUpdateError::StaleVersion {..})=>false,
            Err(SceneUpdateError::InvalidSuccessor {..})=>false,
        },
    {
    if !source_ok {return Err(SceneUpdateError::SourceRejected);}
    if !candidate_ready {
        return Err(SceneUpdateError::NativeRejected);
    }
    Ok(())
}

}
