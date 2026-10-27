// Capability follow-up only; NOT a production proof or replacement implementation.
use vstd::prelude::*;
verus! {
fn admitted_scale(scale: f32) -> (result: bool)
    ensures result == (scale >= 0.015625f32 && scale <= 64.0f32),
{
    scale >= 0.015625f32 && scale <= 64.0f32
}
}
