use vstd::prelude::*;
verus! {
#[derive(PartialEq,Eq)]
enum ImportOutcome {Accepted,Rejected}
spec fn import_outcome(valid:bool)->ImportOutcome {
    if valid {ImportOutcome::Accepted} else {ImportOutcome::Rejected}
}
spec fn forwarded_outcome(valid:bool)->ImportOutcome {import_outcome(valid)}
proof fn forwarding_preserves_import(valid:bool)
    ensures forwarded_outcome(valid)==ImportOutcome::Accepted,
{}
}
