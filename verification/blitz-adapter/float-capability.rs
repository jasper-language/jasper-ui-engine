// Tool capability probe only; NOT a production implementation proof.
use vstd::prelude::*;
verus! {
fn admitted_scale(scale: f32) -> (result: bool)
    ensures result ==> scale >= 0.015625f32 && scale <= 64.0f32,
{
    scale.is_finite() && (0.015625f32..=64.0f32).contains(&scale)
}
}
