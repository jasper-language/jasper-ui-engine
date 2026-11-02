use vstd::prelude::*;
verus! {

/// Highest CSS weight that still gets a synthetic bold stand-in: above it CSS matching
/// already selects the face itself for bold requests.
/// 仍需合成粗体替身的最高 CSS 字重：高于它时 CSS 匹配本就为粗体请求选中该字面自身。
pub const BOLD_ALIAS_MAX_SOURCE_WEIGHT: u16 = 500;

/// One blob's faces, in order: `weights[i]` is face i's CSS weight and `group_max[i]` the
/// heaviest weight in face i's (family, style, width) group. The blob gets a stand-in only when
/// it holds at least one face, both lists agree in length, and every face is the heaviest of its
/// group with that weight at most BOLD_ALIAS_MAX_SOURCE_WEIGHT.
/// 一个 Blob 的各字面按序：`weights[i]` 为字面 i 的 CSS 字重，`group_max[i]` 为其
/// （族、样式、宽度）组中最重字重。仅当 Blob 至少含一个字面、两表等长、且每个字面都是
/// 本组最重且字重不超过 BOLD_ALIAS_MAX_SOURCE_WEIGHT 时，才为该 Blob 做替身。
pub fn bold_alias_admits(weights: &[u16], group_max: &[u16]) -> (result: bool)
    ensures result == (1 <= weights.len() && weights.len() == group_max.len()
        && forall|j: int| 0 <= j < weights.len() ==>
            weights@[j] == group_max@[j] && group_max@[j] <= 500),
{
    if weights.len() < 1 || weights.len() != group_max.len() {
        assert(!(1 <= weights.len() && weights.len() == group_max.len()));
        return false;
    }
    let mut index: usize = 0;
    while index < weights.len()
        invariant index <= weights.len(), weights.len() == group_max.len(),
            forall|j: int| 0 <= j < index ==> weights@[j] == group_max@[j] && group_max@[j] <= 500,
        decreases weights.len() - index,
    {
        if weights[index] != group_max[index] || group_max[index] > BOLD_ALIAS_MAX_SOURCE_WEIGHT {
            assert(weights@[index as int] != group_max@[index as int]
                || group_max@[index as int] > 500);
            return false;
        }
        index += 1;
    }
    true
}

}
