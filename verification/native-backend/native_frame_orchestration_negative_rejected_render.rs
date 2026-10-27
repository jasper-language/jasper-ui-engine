use vstd::prelude::*;
verus! {
spec fn admitted(owner_ok:bool,version_ok:bool,open:bool,extent_ok:bool)->bool {
    owner_ok && version_ok && open && extent_ok
}
proof fn rejected_frame_may_render(owner_ok:bool,version_ok:bool,open:bool,extent_ok:bool)
    ensures !admitted(owner_ok,version_ok,open,extent_ok) ==> admitted(owner_ok,version_ok,open,extent_ok),
{}
}
