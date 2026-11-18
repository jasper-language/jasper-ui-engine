use vstd::prelude::*;
verus! {


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PermissionFailure {
    UnknownContext,
    Denied,
}

pub(crate) fn permission_failure(context_named: bool, permission_granted: bool) -> (result: Option<PermissionFailure>)
    ensures match result {
        Some(PermissionFailure::UnknownContext) => !context_named,
        Some(PermissionFailure::Denied) => context_named && !permission_granted,
        None => context_named && permission_granted,
    },
{
    if !context_named {
        return Some(PermissionFailure::UnknownContext);
    }
    if !context_named {
        return Some(PermissionFailure::Denied);
    }
    None
}

}
