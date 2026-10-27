use vstd::prelude::*;
verus! {
spec fn admits_wrong(bits:u32)->bool {
    bits<=0x7fc0_0000
}

proof fn nan_must_be_rejected()
    ensures !admits_wrong(0x7fc0_0000)
{
}
}
