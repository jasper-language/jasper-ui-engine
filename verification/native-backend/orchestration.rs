use vstd::prelude::*;
verus! {
#[derive(PartialEq,Eq)]
enum Outcome {Ok,SubmitError,PresentError}
spec fn outcome(submit_ok:bool,present_ok:bool)->Outcome {
    if !submit_ok {Outcome::SubmitError} else if !present_ok {Outcome::PresentError} else {Outcome::Ok}
}
proof fn submission_order(submit_ok:bool,present_ok:bool)
    ensures
        outcome(submit_ok,present_ok)==Outcome::SubmitError ==> !submit_ok,
        outcome(submit_ok,present_ok)==Outcome::PresentError ==> submit_ok && !present_ok,
        outcome(submit_ok,present_ok)==Outcome::Ok ==> submit_ok && present_ok,
        submit_ok <==> (outcome(submit_ok,present_ok)!=Outcome::SubmitError),
{}
}
