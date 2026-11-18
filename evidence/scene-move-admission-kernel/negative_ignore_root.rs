use vstd::prelude::*;
verus! {



#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

pub enum SceneError {
    NodeNotFound(NodeId),
    DuplicateNode(NodeId),
    CannotRemoveRoot,
    CannotMoveRoot,
    MoveIntoDescendant(NodeId),
    NotText(NodeId),
    NotCanvas(NodeId),
    StaleVersion {
        expected: u64,
        actual: u64,
    },
    /// No fresh version is available; the scene remains unchanged.
    /// 无可用的新版本；场景保持不变。
    VersionExhausted,
}

fn scene_move_failure(
    root_matches: bool,
    id_present: bool,
    parent_present: bool,
    parent_in_moved: bool,
    id: NodeId,
    parent: NodeId,
) -> (result: Option<SceneError>)
    ensures match result {
        Some(SceneError::CannotMoveRoot) => root_matches,
        Some(SceneError::NodeNotFound(missing)) => !root_matches && (
            (!id_present && missing == id)
            || (id_present && !parent_present && missing == parent)),
        Some(SceneError::MoveIntoDescendant(moved)) =>
            !root_matches && id_present && parent_present && parent_in_moved && moved == id,
        None => !root_matches && id_present && parent_present && !parent_in_moved,
        Some(_) => false,
    },
{
    if false {
        Some(SceneError::CannotMoveRoot)
    } else if !id_present {
        Some(SceneError::NodeNotFound(id))
    } else if !parent_present {
        Some(SceneError::NodeNotFound(parent))
    } else if parent_in_moved {
        Some(SceneError::MoveIntoDescendant(id))
    } else {
        None
    }
}

}
