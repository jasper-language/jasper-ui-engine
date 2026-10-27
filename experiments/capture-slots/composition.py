"""Compose the real candidate method contracts for the original two-key goal.
组合真实候选方法契约；不替代 Kani，也不使用模型容器。
"""
from pathlib import Path
import runpy
root = Path(__file__).resolve().parents[2]
runpy.run_path(str(root/'experiments/capture-slots/probe.py'))
base=(root/'evidence/capture-slots-candidate/positive.rs').read_text()
harness=r'''
// Executable verification harness calling actual candidate methods.
// 验证入口调用原候选方法，无额外业务假设；输入键和目标均不受限。
fn two_key_query(a: u64, b: u64, q: u64, x: Capture, y: Capture)
    -> (result: Option<Option<NodeId>>)
    ensures result == if q == b { Some(y.target) }
        else if q == a { Some(x.target) } else { None },
{
    let mut slots = CaptureSlots::new();
    let first = slots.insert(a, x);
    assert(first.is_none());
    assert(slots.entries@ =~= seq![(a, x)]);
    let ghost before = slots.entries@;
    let previous = slots.insert(b, y);
    proof {
        match previous {
            Some(_) => {
                let i = choose|i: int| 0 <= i < before.len()
                    && before[i].0 == b
                    && slots.entries@ == before.update(i, (b, y));
                assert(i == 0);
                assert(a == b);
                assert(slots.entries@ == before.update(0, (b, y)));
            },
            None => { assert(before[0].0 != b); assert(a != b); },
        }
        if a == b {
            assert(slots.entries@ =~= seq![(b, y)]);
        } else {
            assert(slots.entries@ =~= seq![(a, x), (b, y)]);
        }
    }
    assert(previous.is_some() == (a == b));
    assert(slots.entries@.len() == if a == b { 1int } else { 2int });
    let ghost entries = slots.entries@;
    match slots.get(q) {
        Some(capture) => {
            proof {
                let i = choose|i: int| 0 <= i < entries.len()
                    && entries[i].0 == q && *capture == entries[i].1;
                if a == b { assert(i == 0); }
                else { assert(i == 0 || i == 1); }
                if q == b { assert(capture.target == y.target); }
                else { assert(q == a); assert(capture.target == x.target); }
            }
            Some(capture.target)
        },
        None => {
            proof {
                assert(entries[0].0 != q);
                if a != b { assert(entries[1].0 != q); }
                assert(q != b); assert(q != a);
            }
            None
        },
    }
}
'''
prefix,suffix=base.rsplit('\n}\nfn main() {}',1)
out=root/'evidence/capture-composition';out.mkdir(exist_ok=True)
(out/'positive.rs').write_text(prefix+harness+'\n}\nfn main() {}'+suffix)
(out/'wrong-target.rs').write_text((out/'positive.rs').read_text().replace('Some(capture.target)', 'Some(None)'))
