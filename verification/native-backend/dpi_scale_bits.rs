use vstd::prelude::*;
verus! {
const MIN_DPI_BITS:u32=0x3c80_0000;
const MAX_DPI_BITS:u32=0x4280_0000;

spec fn admits(bits:u32)->bool {
    MIN_DPI_BITS<=bits && bits<=MAX_DPI_BITS
}

proof fn dpi_interval_partition(bits:u32)
    ensures
        admits(bits) <==> MIN_DPI_BITS<=bits<=MAX_DPI_BITS,
        admits(bits) ==> bits<0x8000_0000,
        bits<MIN_DPI_BITS ==> !admits(bits),
        bits>MAX_DPI_BITS ==> !admits(bits),
        admits(MIN_DPI_BITS),
        admits(MAX_DPI_BITS),
{
}
}
