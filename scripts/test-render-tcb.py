#!/usr/bin/env python3
"""Each way the register can drift must be reported. 登记表每种漂移都必须被报告。"""
import importlib.util, re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("check_render_tcb", ROOT / "scripts/check-render-tcb.py")
tcb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tcb)
manifest = (ROOT / "Cargo.toml").read_text()
lock = (ROOT / "Cargo.lock").read_text()
doc = (ROOT / "docs/render-tcb.md").read_text()

errors, _ = tcb.check(manifest, lock, doc)
assert not errors, errors

def reported(name, manifest=manifest, lock=lock, doc=doc):
    errors, _ = tcb.check(manifest, lock, doc)
    assert errors, name + " was not reported"

wgpu_row = re.search(r"^\| `wgpu` \| `[^`]+` \|.*\n", doc, re.M).group(0)
reported("row removed", doc=doc.replace(wgpu_row, ""))
reported("required transitive removed", doc=re.sub(r"^\| `wgpu-hal` .*\n", "", doc, flags=re.M))
reported("version stale", doc=doc.replace("| `wgpu` | `30.0.1` |", "| `wgpu` | `30.0.0` |"))
reported("row for a crate outside the closure", doc=doc + "\n| `not-a-crate` | `1.0.0` | x |\n")
reported("row twice", doc=doc + "\n" + wgpu_row)
reported("count edited", doc=re.sub(r"闭包数量：\d+", "闭包数量：1", doc))
reported("digest edited", doc=re.sub(r"闭包摘要：`[0-9a-f]{64}`", "闭包摘要：`" + "0" * 64 + "`", doc))
reported("lock version moved", lock=lock.replace('name = "wgpu-hal"\nversion = "30.0.1"', 'name = "wgpu-hal"\nversion = "30.0.2"'))
reported("new direct dependency", manifest=manifest.replace('gpu-pending = ["cpu-pending", ', 'gpu-pending = ["cpu-pending", "dep:libc", '))
print("render TCB drift checks: 9 reported")
