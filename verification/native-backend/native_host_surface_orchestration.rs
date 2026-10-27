use vstd::prelude::*;
verus! {
#[derive(PartialEq,Eq)]
enum Outcome {SurfaceError,RenderError,PackedLengthError,Copied}

spec fn outcome(surface_ok:bool,render_ok:bool,packed_exact:bool)->Outcome {
    if !surface_ok {Outcome::SurfaceError}
    else if !render_ok {Outcome::RenderError}
    else if !packed_exact {Outcome::PackedLengthError}
    else {Outcome::Copied}
}

proof fn host_surface_order(surface_ok:bool,render_ok:bool,packed_exact:bool)
    ensures
        !surface_ok ==> outcome(surface_ok,render_ok,packed_exact)==Outcome::SurfaceError,
        outcome(surface_ok,render_ok,packed_exact)==Outcome::SurfaceError ==> !surface_ok,
        outcome(surface_ok,render_ok,packed_exact)==Outcome::RenderError
            ==> surface_ok && !render_ok,
        outcome(surface_ok,render_ok,packed_exact)==Outcome::PackedLengthError
            ==> surface_ok && render_ok && !packed_exact,
        outcome(surface_ok,render_ok,packed_exact)==Outcome::Copied
            ==> surface_ok && render_ok && packed_exact,
{
}
}
