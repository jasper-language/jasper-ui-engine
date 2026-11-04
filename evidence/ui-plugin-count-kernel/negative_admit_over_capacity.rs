use vstd::prelude::*;
verus! {

/// The command count from an entry's return value, when it fits the buffer.
/// 入口返回值所表示的命令数（须不超过缓冲区容量）。
pub fn paint_count_admits(returned: i32, capacity: u32) -> (result: Option<u32>)
    ensures match result {
        Some(count) => returned >= 0 && count as int == returned as int && count <= capacity,
        None => returned < 0 || returned as int > capacity as int,
    },
{
    if returned < 0 {
        return None;
    }
    let count = returned as u32;
    if count > capacity + 1 {
        return None;
    }
    Some(count)
}

}
