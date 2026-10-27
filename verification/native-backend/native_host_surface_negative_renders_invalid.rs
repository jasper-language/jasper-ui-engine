use vstd::prelude::*;
verus! {
spec fn render_called_wrong(surface_ok:bool)->bool { true }

proof fn invalid_surface_must_not_render(surface_ok:bool)
    ensures !surface_ok ==> !render_called_wrong(surface_ok),
{
}
}
