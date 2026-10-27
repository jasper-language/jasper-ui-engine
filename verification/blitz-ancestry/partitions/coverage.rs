use vstd::prelude::*;
verus! {
// Exact scalar value produced by the unchanged Kani harness's parent closure.
// 原始 Kani 父边闭包的标量结果；不建模或替换实际 BTreeMap。
pub open spec fn decoded_parent(raw:int)->Option<int> {
    if raw % 4 == 0 {None} else {Some(raw % 4 - 1)}
}
proof fn partition_covers_original_bytes(a:u8,b:u8,start:u8,target:u8)
    ensures
        0 <= (a as int) % 4 < 4,
        0 <= (b as int) % 4 < 4,
        decoded_parent(a as int) == decoded_parent((a as int) % 4),
        decoded_parent(b as int) == decoded_parent((b as int) % 4),
        0 <= (start as int) % 4 < 4,
        0 <= (target as int) % 4 < 4,
        ((start as int) % 4) % 4 == (start as int) % 4,
        ((target as int) % 4) % 4 == (target as int) % 4,
{}
}
