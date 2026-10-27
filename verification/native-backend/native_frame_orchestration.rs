use vstd::prelude::*;
verus! {
#[derive(PartialEq,Eq)]
enum Outcome {AdmissionError,RendererError,PackedLengthError,SubmitError,PresentError,Ok}

spec fn admitted(owner_ok:bool,version_ok:bool,open:bool,extent_ok:bool)->bool {
    owner_ok && version_ok && open && extent_ok
}

spec fn outcome(owner_ok:bool,version_ok:bool,open:bool,extent_ok:bool,
    renderer_ok:bool,packed_exact:bool,submit_ok:bool,present_ok:bool)->Outcome {
    if !admitted(owner_ok,version_ok,open,extent_ok) {Outcome::AdmissionError}
    else if !renderer_ok {Outcome::RendererError}
    else if !packed_exact {Outcome::PackedLengthError}
    else if !submit_ok {Outcome::SubmitError}
    else if !present_ok {Outcome::PresentError}
    else {Outcome::Ok}
}

proof fn native_frame_order(owner_ok:bool,version_ok:bool,open:bool,extent_ok:bool,
    renderer_ok:bool,packed_exact:bool,submit_ok:bool,present_ok:bool)
    ensures
        admitted(owner_ok,version_ok,open,extent_ok)
            <==> outcome(owner_ok,version_ok,open,extent_ok,renderer_ok,packed_exact,submit_ok,present_ok)!=Outcome::AdmissionError,
        outcome(owner_ok,version_ok,open,extent_ok,renderer_ok,packed_exact,submit_ok,present_ok)==Outcome::RendererError
            ==> admitted(owner_ok,version_ok,open,extent_ok) && !renderer_ok,
        outcome(owner_ok,version_ok,open,extent_ok,renderer_ok,packed_exact,submit_ok,present_ok)==Outcome::PackedLengthError
            ==> admitted(owner_ok,version_ok,open,extent_ok) && renderer_ok && !packed_exact,
        outcome(owner_ok,version_ok,open,extent_ok,renderer_ok,packed_exact,submit_ok,present_ok)==Outcome::SubmitError
            ==> admitted(owner_ok,version_ok,open,extent_ok) && renderer_ok && packed_exact && !submit_ok,
        outcome(owner_ok,version_ok,open,extent_ok,renderer_ok,packed_exact,submit_ok,present_ok)==Outcome::PresentError
            ==> admitted(owner_ok,version_ok,open,extent_ok) && renderer_ok && packed_exact && submit_ok && !present_ok,
        outcome(owner_ok,version_ok,open,extent_ok,renderer_ok,packed_exact,submit_ok,present_ok)==Outcome::Ok
            ==> admitted(owner_ok,version_ok,open,extent_ok) && renderer_ok && packed_exact && submit_ok && present_ok,
{}
}
