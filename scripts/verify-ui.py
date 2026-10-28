#!/usr/bin/env python3
"""Ordered UI validation; missing harnesses and tools never count as success.
顺序执行 UI 验证；缺工具或缺验证入口均不得计为通过。
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import re
import shutil
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument("--verus", required=True)
p.add_argument("--nightly", default="nightly-2026-10-17")
p.add_argument("--fuzz-seconds", type=int, default=20)
a = p.parse_args()
if a.fuzz_seconds < 1:
    p.error("--fuzz-seconds must be positive")
os.chdir(ROOT)
out = ROOT / "evidence" / "validation-chain"
out.mkdir(parents=True, exist_ok=True)
report = {"started_at": datetime.now(timezone.utc).isoformat(), "scope": "six-stage read-boundary, version admission and surface row/span/borrow/write and scalar/header/payload-read and checked surface view/transfer and RGB packing/conversion and owned Canvas import/getter and Canvas pixel read and native font stack admission/fallback order and on-demand frame reuse admission and frame damage span/scan/rect admission/damage-only surface write and refresh-clock frame tick/pacing and surface segment write and resource gate/byte storage/keyboard role/modifier and approximate text classifier and focus cleanup/retirement slices; additional batch checks have partial proof coverage",
          "slice_complete": False, "scoped_checks_complete": False,
          "acceptance_pending": ["capture-query: original arbitrary two-key Kani obligation is unresolved; single-key harness is diagnostic only"],
          "sources": {}, "stages": [], "complete": False}
for path in sorted(list((ROOT / "src").glob("*.rs")) + list((ROOT / "tests").glob("*.rs"))
                   + list((ROOT / "scripts").glob("*.py"))
                   + [p for base in ["verification", "fuzz"] for p in (ROOT / base).rglob("*")
                      if p.is_file() and not any(part in {"target", "corpus", "artifacts"} for part in p.relative_to(ROOT).parts)]
                   + [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]):
    report["sources"][str(path.relative_to(ROOT))] = hashlib.sha256(path.read_bytes()).hexdigest()

def save():
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")

# Write an incomplete report before attempting anything, invalidating old success.
# 执行前先写未完成报告，撤销旧成功状态。
save()
for old_log in ["Verus-Z3.log", "Proptest.log", "Miri.log", "cargo-fuzz.log", "Kani.log", "Loom.log", "kani-source-map.json"]:
    (out / old_log).unlink(missing_ok=True)
# Archive the exact same production regions consumed by the verification crate.
# 归档验证 crate 使用的同一生产区域，便于独立核对提取映射。
regions = []
for file, markers in [
    ("src/batch.rs", ["DECODE RANGE KERNEL", "BYTE READ KERNEL", "BATCH ERROR TYPE", "CURSOR READ KERNEL", "SCALAR READ KERNEL", "HEADER CONSTANTS TYPE", "HEADER READ KERNEL", "BOUNDED PAYLOAD KERNEL"]),
    ("src/surface.rs", ["SURFACE ROW KERNEL", "SURFACE SPAN KERNEL", "SURFACE BORROW KERNEL", "SURFACE WRITE KERNEL", "SURFACE SEGMENT WRITE KERNEL", "SURFACE VIEW KERNEL", "PIXEL PACK KERNEL"]),
    ("src/geometry.rs", ["GEOMETRY POINT TYPE", "GEOMETRY SIZE TYPE", "GEOMETRY RECT TYPE"]),
    ("src/raster.rs", ["CANVAS STORAGE TYPE", "CANVAS PIXEL READ KERNEL", "CANVAS IMPORT KERNEL"]),
    ("src/style.rs", ["COLOR TYPE"]),
    ("src/native_adapter/native_fonts.rs", ["FONT ORDER KERNEL"]),
    ("src/native_adapter/frame_schedule.rs", ["FRAME REUSE KERNEL", "FRAME DAMAGE TYPE", "DAMAGE SPAN KERNEL", "FRAME DAMAGE KERNEL", "DAMAGE RECT KERNEL", "DAMAGE WRITE KERNEL", "FRAME TICK KERNEL", "FRAME PACE TYPE", "FRAME PACE KERNEL"]),
    ("src/text.rs", ["TEXT CLASS KERNEL"]),
    ("src/native_adapter/text_geometry.rs", ["TEXT OFFSET KERNEL", "TEXT RANGE KERNEL"]),
    ("src/native_adapter/access_tree.rs", ["ACCESS ACTIONS TYPE", "ACCESS ACTIONS KERNEL", "ACCESS CHANGE TYPE", "ACCESS CHANGE KERNEL"]),
    ("src/native_adapter/gpu_render.rs", ["GPU READBACK LAYOUT KERNEL"]),
    ("src/resource.rs", ["RESOURCE GATE KERNEL", "BYTES SLOT KERNEL"]),
    ("src/input.rs", ["MODIFIERS TYPE", "KEY PHASE TYPE", "KEY EVENT TYPE", "POINTER ID TYPE", "POINTER ORDER KERNEL", "POINTER KIND TYPES", "POINTER EVENT TYPE", "POINTER TRACKER TYPE"]),
    ("src/semantics.rs", ["ROLE TYPE", "FOCUS STORAGE TYPE", "FOCUS CLEANUP KERNEL", "FOCUS ERROR TYPE", "FOCUS SELECT KERNEL"]),
    ("src/keyboard.rs", ["ROLE ACTIVATION KERNEL", "COMMAND MODIFIER KERNEL", "KEYBOARD STORAGE TYPE", "KEYBOARD FOCUS KERNEL", "INPUT RETIRE KERNEL"]),
    ("src/pointer.rs", ["POINTER ROUTER TYPE", "CAPTURE QUERY KERNEL", "HOVER CHANGE TYPE", "HOVER STORAGE TYPE", "HOVER UPDATE KERNEL", "HOVER RETIRE KERNEL"]),
    ("src/desktop.rs", ["PIXEL ERROR TYPE", "PIXEL CONVERT KERNEL"]),
    ("src/scene.rs", ["NODE ID TYPE", "VERSION KERNEL", "TRANSACTION GATE KERNEL"]),
]:
    source = (ROOT / file).read_text()
    for marker in markers:
        begin, end = f"// BEGIN VERIFIED {marker}", f"// END VERIFIED {marker}"
        if source.count(begin) != 1 or source.count(end) != 1:
            raise RuntimeError("ambiguous production marker " + marker)
        regions.append(source.split(begin)[1].split(end)[0])
(out / "production.rs").write_text("".join(regions))
report["production_regions_sha256"] = hashlib.sha256((out / "production.rs").read_bytes()).hexdigest()
manifest = "verification/read-boundary/Cargo.toml"
corpora = out / "corpora"
for target in ["read_boundary", "batch_decode"]:
    (corpora / target).mkdir(parents=True, exist_ok=True)
    (out / "fuzz-artifacts" / target).mkdir(parents=True, exist_ok=True)
    for seed in (ROOT / "fuzz/seeds" / target).iterdir():
        shutil.copyfile(seed, corpora / target / seed.name)
kernels = ["version", "decode-range", "transaction-gate", "byte-read", "cursor-read",
           "scalar-read", "header-read", "bounded-payload", "surface-row", "surface-span", "surface-borrow", "surface-write", "surface-segment", "surface-view", "pixel-pack", "pixel-convert", "canvas-import", "canvas-pixel", "font-order", "frame-reuse", "damage-span", "damage-rect", "frame-tick", "frame-pace", "text-offset", "text-range", "access-actions", "access-change", "gpu-readback", "resource-gate", "role-activation", "bytes-slot", "command-modifier", "text-class", "focus-cleanup", "keyboard-focus", "focus-select", "input-retire", "hover-update", "hover-retire", "pointer-order", "capture-query"]
stages = [
    ("Verus+Z3", [[sys.executable, "scripts/verify-version.py", "--kernel", k, "--verus", a.verus] for k in kernels]),
    ("Proptest", [["cargo", "--version"], ["cargo", "test", "--locked", "--test", "batch_properties"],
                  ["cargo", "test", "--locked", "--test", "canvas_import"],
                  ["cargo", "test", "--locked", "--test", "resource_gate"],
                  ["cargo", "test", "--locked", "--test", "bytes_slot"],
                  ["cargo", "test", "--locked", "--manifest-path", manifest, "properties::"],
                  ["cargo", "test", "--locked", "--features", "desktop", "--lib", "desktop::tests"]]),
    ("Miri", [["cargo", "+" + a.nightly, "miri", "--version"], ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "batch_memory"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--manifest-path", manifest, "memory::"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "canvas_import"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "resource_gate"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "bytes_slot"]]),
    ("cargo-fuzz", [["cargo", "fuzz", "--version"]] + [
        ["cargo", "+" + a.nightly, "fuzz", "run", target, str(corpora / target), "--",
         "-max_total_time=" + str(a.fuzz_seconds), "-max_len=4096", "-rss_limit_mb=1024",
         "-timeout=5", "-seed=12345", "-artifact_prefix=" + str(out / "fuzz-artifacts" / target) + "/"] for target in ["read_boundary", "batch_decode"]]),
    ("Kani", [["cargo", "kani", "--version"],
              ["cargo", "kani", "--manifest-path", manifest]]),
    ("Loom", [["cargo", "test", "--locked", "--manifest-path", manifest,
               "schedules::shared_immutable_buffer_independent_cursors", "--", "--exact"]]),
]
for name, commands in stages:
    stage = {"name": name, "status": "not_run", "commands": commands}
    report["stages"].append(stage)
save()
for stage in report["stages"]:
    if stage["commands"] is None:
        stage.update(status="not_implemented", reason="Scoped production harness / applicability review pending")
        save()
        print(stage["name"] + ": incomplete; subsequent stages not run", flush=True)
        sys.exit(2)
    stage["started_at"] = datetime.now(timezone.utc).isoformat()
    stage["status"] = "running"
    save()
    environment = os.environ.copy()
    if stage["name"] == "cargo-fuzz":
        # Retain the actual starting corpus, not only the checked-in seeds.
        # 保存实际初始语料，而不仅是仓库种子；崩溃输入同样落到 evidence。
        stage["initial_corpus"] = {}
        for target in ["read_boundary", "batch_decode"]:
            snapshot = out / "initial-corpus" / target
            if snapshot.exists():
                shutil.rmtree(snapshot)
            snapshot.mkdir(parents=True)
            stage["initial_corpus"][target] = {}
            for path in sorted((corpora / target).iterdir()):
                if path.is_file():
                    digest = hashlib.sha256(path.read_bytes()).hexdigest()
                    stage["initial_corpus"][target][path.name] = digest
                    shutil.copyfile(path, snapshot / path.name)
        save()
    if stage["name"] == "Proptest":
        stage["environment"] = {"PROPTEST_RNG_SEED": "12345"}
        environment.update(stage["environment"])
        save()
    if stage["name"] == "Kani":
        # Content-addressed cache: stale OUT_DIR includes must never substitute for current code.
        # 按源码内容隔离缓存，避免旧 OUT_DIR 提取文件冒充当前实现。
        fingerprint = hashlib.sha256(json.dumps(report["sources"], sort_keys=True).encode()).hexdigest()
        stage["environment"] = {"CARGO_TARGET_DIR": str(ROOT / "target/ui-kani" / fingerprint)}
        environment.update(stage["environment"])
        save()
    with (out / (stage["name"].replace("+", "-") + ".log")).open("w") as log:
        try:
            for command in stage["commands"]:
                log.write(json.dumps(command) + "\n")
                log.flush()
                result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=1800, env=environment)
                if result.returncode:
                    raise RuntimeError("command exit " + str(result.returncode))
        except (OSError, subprocess.TimeoutExpired, RuntimeError) as exc:
            stage.update(status="failed", reason=str(exc))
            save()
            print(stage["name"] + ": " + str(exc), flush=True)
            sys.exit(1)
    log_path = out / (stage["name"].replace("+", "-") + ".log")
    output = log_path.read_text()
    required = {
        "Proptest": [("test result: ok. 2 passed", 3), ("test result: ok. 3 passed", 1), ("test result: ok. 42 passed", 1), ("test result: ok. 6 passed", 1)],
        "Miri": [("test result: ok. 2 passed", 4), ("test result: ok. 1 passed", 1)],
        "Kani": [("VERIFICATION:- SUCCESSFUL", 47)],
        "Loom": [("test result: ok. 1 passed", 1)],
    }
    matched = all(output.count(token) >= count for token, count in required.get(stage["name"], []))
    if stage["name"] == "cargo-fuzz":
        stage["completed_runs"] = [int(n) for n in re.findall(r"Done (\d+) runs in", output)]
        matched = len(stage["completed_runs"]) == 2 and all(n > 0 for n in stage["completed_runs"])
    if not matched:
        stage.update(status="failed", reason="missing non-vacuous completion evidence")
        save()
        sys.exit(1)
    if stage["name"] == "Kani":
        generated = list(Path(stage["environment"]["CARGO_TARGET_DIR"]).glob("**/out/production.rs"))
        if not generated or any(hashlib.sha256(path.read_bytes()).hexdigest() != report["production_regions_sha256"] for path in generated):
            stage.update(status="failed", reason="generated Kani production body mismatch")
            save()
            sys.exit(1)
        stage["generated_production_sha256"] = report["production_regions_sha256"]
    stage["log_sha256"] = hashlib.sha256(log_path.read_bytes()).hexdigest()
    stage["finished_at"] = datetime.now(timezone.utc).isoformat()
    stage["status"] = "passed_scoped_checks"
    save()
    print(stage["name"] + ": scoped checks passed", flush=True)
changed = [name for name, digest in report["sources"].items()
           if not (ROOT / name).is_file() or hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != digest]
if changed:
    report["changed_during_run"] = changed
    save()
    sys.exit(1)
# Scoped diagnostic success must not override an unresolved required obligation.
# 受限诊断成功不能覆盖尚未完成的强制验证义务。
report["scoped_checks_complete"] = True
report["slice_complete"] = not report["acceptance_pending"]
report["finished_at"] = datetime.now(timezone.utc).isoformat()
# Deliberately no whole-engine completion claim from this initial slice.
# 初始切片不能产生整个引擎验收完成结论。
save()

if report["acceptance_pending"]:
    print("Acceptance incomplete: " + "; ".join(report["acceptance_pending"]), flush=True)
    sys.exit(2)
