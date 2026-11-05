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

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kani_policy

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument("--verus", required=True)
p.add_argument("--nightly", default="nightly-2026-10-17")
p.add_argument("--fuzz-seconds", type=int, default=20)
kani_scope = p.add_mutually_exclusive_group()
kani_scope.add_argument("--kani-harness", action="append", default=[], metavar="NAME",
                        help="release validation only: harness to run; repeat for more. Default runs no Kani")
kani_scope.add_argument("--kani-full", action="store_true", help="release validation only: whole-library Kani")
p.add_argument("--kani-seconds", type=int, default=kani_policy.MAX_SECONDS)
p.add_argument("--kani-vmem-kb", type=int, default=kani_policy.MAX_VMEM_KB)
a = p.parse_args()
if a.fuzz_seconds < 1:
    p.error("--fuzz-seconds must be positive")
if not 1 <= a.kani_seconds <= kani_policy.MAX_SECONDS or not 1 <= a.kani_vmem_kb <= kani_policy.MAX_VMEM_KB:
    p.error("Kani limits may be lowered but never raised")
if len(set(a.kani_harness)) != len(a.kani_harness) or not all(map(kani_policy.valid_harness_name, a.kani_harness)):
    p.error("--kani-harness names must be valid and distinct")
os.chdir(ROOT)
out = ROOT / "evidence" / "validation-chain"
out.mkdir(parents=True, exist_ok=True)
report = {"started_at": datetime.now(timezone.utc).isoformat(), "scope": "six-stage read-boundary, version admission/structured scene-update and component-update rejection/scene-frame completion and surface row/span/borrow/write and scalar/header/payload-read and checked surface view/transfer and RGB packing/conversion and owned Canvas import/getter and Canvas pixel read and native font stack admission/fallback order/bold-alias admission and on-demand frame reuse admission and frame damage span/scan/rect admission/damage-only surface write and refresh-clock frame tick/pacing and surface segment write and resource gate/byte storage/keyboard role/modifier and approximate text classifier and focus cleanup/retirement slices; additional batch checks have partial proof coverage",
          "slice_complete": False, "scoped_checks_complete": False,
          "acceptance_pending": [],
          # Moved out of daily acceptance on 2026-11-04; still open, to be validated at formal release.
          # 2026-11-04 移出日常验收；义务仍未解决，正式版发版时验证。
          "release_checklist": ["CP-01: original arbitrary two-key Kani obligation is unresolved; single-key harness is diagnostic only",
                                "CP-02: real BTreeMap Kani / Bitwuzla path is unresolved"],
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
shutil.rmtree(out / "kani-harness-logs", ignore_errors=True)
# Archive the exact same production regions consumed by the verification crate.
# 归档验证 crate 使用的同一生产区域，便于独立核对提取映射。
regions = []
for file, markers in [
    ("src/batch.rs", ["DECODE RANGE KERNEL", "BYTE READ KERNEL", "BATCH ERROR TYPE", "CURSOR READ KERNEL", "SCALAR READ KERNEL", "HEADER CONSTANTS TYPE", "HEADER READ KERNEL", "BOUNDED PAYLOAD KERNEL"]),
    ("src/surface.rs", ["SURFACE ROW KERNEL", "SURFACE SPAN KERNEL", "SURFACE BORROW KERNEL", "SURFACE WRITE KERNEL", "SURFACE SEGMENT WRITE KERNEL", "SURFACE VIEW KERNEL", "PIXEL PACK KERNEL"]),
    ("src/geometry.rs", ["GEOMETRY POINT TYPE", "GEOMETRY SIZE TYPE", "GEOMETRY RECT TYPE"]),
    ("src/raster.rs", ["CANVAS STORAGE TYPE", "CANVAS PIXEL READ KERNEL", "CANVAS IMPORT KERNEL"]),
    ("src/style.rs", ["COLOR TYPE", "COLOR CONSTRUCT KERNEL"]),
    ("src/native_adapter/native_fonts.rs", ["FONT ORDER KERNEL", "FONT WEIGHT KERNEL", "FONT FAMILY KERNEL", "BOLD ALIAS KERNEL"]),
    ("src/native_adapter/frame_schedule.rs", ["FRAME REUSE KERNEL", "FRAME DAMAGE TYPE", "DAMAGE SPAN KERNEL", "FRAME DAMAGE KERNEL", "DAMAGE RECT KERNEL", "DAMAGE WRITE KERNEL", "FRAME TICK KERNEL", "FRAME PACE TYPE", "FRAME PACE KERNEL"]),
    ("src/text.rs", ["TEXT CLASS KERNEL"]),
    ("src/native_adapter/text_geometry.rs", ["TEXT OFFSET KERNEL", "TEXT RANGE KERNEL"]),
    ("src/native_adapter/access_tree.rs", ["ACCESS ACTIONS TYPE", "ACCESS ACTIONS KERNEL", "ACCESS CHANGE TYPE", "ACCESS CHANGE KERNEL"]),
    ("src/native_adapter/access_kit.rs", ["ACCESS REQUEST TYPE", "ACCESS REQUEST KERNEL"]),
    ("src/native_adapter/gpu_render.rs", ["GPU READBACK LAYOUT KERNEL"]),
    ("src/native_adapter/glyph_skew.rs", ["GLYPH SKEW KERNEL"]),
    ("src/native_adapter/native_scene.rs", ["SCENE UPDATE ERROR TYPE KERNEL", "SCENE FRAME COMPLETION KERNEL", "SCENE FRAME ORDER KERNEL", "SCENE INTERACTION RESTORE KERNEL"]),
    ("src/resource.rs", ["RESOURCE GATE KERNEL", "BYTES SLOT KERNEL"]),
    ("src/ui_plugin_gate.rs", ["UI PLUGIN RECT KERNEL", "UI PLUGIN COUNT KERNEL", "UI PLUGIN RELOC KERNEL"]),
    ("src/surface_lifecycle.rs", ["SURFACE STATE TYPE", "SURFACE STATUS TYPE", "SURFACE LIFECYCLE ERROR TYPE", "SURFACE LIFECYCLE STORAGE TYPE", "SURFACE RENDER GATE KERNEL", "SURFACE TRANSITION KERNEL", "SURFACE STEP ERROR TYPE", "SURFACE COORDINATOR STORAGE TYPE", "SURFACE COORDINATION KERNEL"]),
    ("src/input.rs", ["MODIFIERS TYPE", "KEY PHASE TYPE", "KEY EVENT TYPE", "POINTER ID TYPE", "POINTER ORDER KERNEL", "POINTER KIND TYPES", "POINTER EVENT TYPE", "POINTER TRACKER TYPE"]),
    ("src/semantics.rs", ["ROLE TYPE", "FOCUS STORAGE TYPE", "FOCUS CLEANUP KERNEL", "FOCUS ERROR TYPE", "FOCUS SELECT KERNEL"]),
    ("src/keyboard.rs", ["ROLE ACTIVATION KERNEL", "COMMAND MODIFIER KERNEL", "KEYBOARD STORAGE TYPE", "KEYBOARD FOCUS KERNEL", "INPUT RETIRE KERNEL"]),
    ("src/pointer.rs", ["POINTER ROUTER TYPE", "CAPTURE QUERY KERNEL", "HOVER CHANGE TYPE", "HOVER STORAGE TYPE", "HOVER UPDATE KERNEL", "HOVER RETIRE KERNEL"]),
    ("src/desktop.rs", ["PIXEL ERROR TYPE", "PIXEL CONVERT KERNEL", "DESKTOP POINTER KERNEL", "DESKTOP POINTER STATE TYPE", "DESKTOP POINTER STATE KERNEL"]),
    ("src/handle.rs", ["HANDLE TYPE", "HANDLE KERNEL"]),
    ("src/scene.rs", ["NODE ID TYPE", "VERSION KERNEL", "TRANSACTION GATE KERNEL", "SCENE BATCH STAGE KERNEL", "SCENE STATE TYPE", "SCENE BATCH COMMIT KERNEL", "SCENE NODE KIND TYPE", "SCENE NODE TYPE", "SCENE PATCH TYPE", "SCENE ERROR TYPE", "SCENE NODE REMOVE KERNEL", "SCENE NODE FIND KERNEL", "SCENE PATCH KERNEL", "SCENE CHECKED PATCH KERNEL", "SCENE BATCH UNIQUE KERNEL", "SCENE UNIQUE IDS KERNEL", "SCENE IDENTITY SET INSTANCE"]),
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
host_surface_manifest = "verification/host-surface/Cargo.toml"
FUZZ_TARGETS = ["read_boundary", "batch_decode", "ui_plugin_gate", "host_surface", "host_coordination", "host_render"]
corpora = out / "corpora"
for target in FUZZ_TARGETS:
    (corpora / target).mkdir(parents=True, exist_ok=True)
    (out / "fuzz-artifacts" / target).mkdir(parents=True, exist_ok=True)
    for seed in (ROOT / "fuzz/seeds" / target).iterdir():
        shutil.copyfile(seed, corpora / target / seed.name)
kernels = ["version", "decode-range", "transaction-gate", "byte-read", "cursor-read",
           "scalar-read", "header-read", "bounded-payload", "surface-row", "surface-span", "surface-borrow", "surface-write", "surface-segment", "surface-view", "pixel-pack", "pixel-convert", "canvas-import", "canvas-pixel", "color-construct", "desktop-pointer", "desktop-pointer-state", "handle", "font-order", "font-weight", "font-family", "bold-alias", "glyph-skew", "frame-reuse", "damage-span", "damage-rect", "frame-tick", "frame-pace", "text-offset", "text-range", "access-actions", "access-change", "access-request", "gpu-readback", "resource-gate", "role-activation", "bytes-slot", "command-modifier", "text-class", "focus-cleanup", "keyboard-focus", "focus-select", "input-retire", "hover-update", "hover-retire", "pointer-order", "capture-query", "scene-update-admission", "scene-frame-completion", "scene-frame-order", "scene-batch-stage", "scene-node-remove", "scene-node-find", "scene-patch", "scene-unique-ids", "scene-checked-patch", "scene-batch-unique", "scene-batch-commit", "scene-interaction-restore", "component-update-admission", "ui-plugin-rect", "ui-plugin-count", "ui-plugin-reloc", "surface-render-gate", "surface-transition", "surface-coordination", "surface-render-outcome", "capability-matrix"]
if a.kani_full:
    # Sequential whole-library run keeps the shared lock and memory ceiling; the per-harness clock does not apply.
    # 整库顺序运行保留共享锁与内存上限；单 harness 时钟不适用。
    kani_commands = [["flock", kani_policy.LOCK, "bash", "-c", 'ulimit -v "$1"; shift; "$@"', "kani-limit",
                      str(a.kani_vmem_kb), "cargo", "kani", "--manifest-path", manifest]]
else:
    kani_commands = [kani_policy.command(manifest, harness, a.kani_seconds, a.kani_vmem_kb) for harness in a.kani_harness]
report["kani_scope"] = {"mode": "full" if a.kani_full else "targeted" if a.kani_harness else "release_only",
                        "harnesses": a.kani_harness, "seconds": a.kani_seconds, "vmem_kb": a.kani_vmem_kb,
                        "record": "Kani：发版时验证" if not (a.kani_full or a.kani_harness) else "explicit release validation run",
                        "other_harnesses": "not run here; prior evidence stays bound to its own commit"}
stages = [
    ("Verus+Z3", [[sys.executable, "scripts/verify-version.py", "--kernel", k, "--verus", a.verus] for k in kernels]),
    ("Proptest", [["cargo", "--version"], ["cargo", "test", "--locked", "--test", "batch_properties"],
                  ["cargo", "test", "--locked", "--test", "canvas_import"],
                  ["cargo", "test", "--locked", "--test", "resource_gate"],
                  ["cargo", "test", "--locked", "--test", "bytes_slot"],
                  ["cargo", "test", "--locked", "--test", "scene_batch_transaction"],
                  ["cargo", "test", "--locked", "--test", "ui_plugin_gate"],
                  ["cargo", "test", "--locked", "--manifest-path", host_surface_manifest],
                  ["cargo", "test", "--locked", "--manifest-path", manifest, "properties::"],
                  ["cargo", "test", "--locked", "--features", "desktop", "--lib", "desktop::tests"]]),
    ("Miri", [["cargo", "+" + a.nightly, "miri", "--version"], ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "batch_memory"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--manifest-path", manifest, "memory::"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "canvas_import"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "resource_gate"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "bytes_slot"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "handle_table"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "handle_alloc_failure"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "scene_batch_transaction"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--no-default-features", "--test", "ui_plugin_gate"],
               ["cargo", "+" + a.nightly, "miri", "test", "--locked", "--manifest-path", host_surface_manifest, "--",
                "--skip", "any_state_follows_the_table", "--skip", "a_live_generation_names_one_state_along_any_trace",
                "--skip", "successful_cycles_stay_active_and_monotone",
                "--skip", "a_pending_present_belongs_to_the_generation_that_rendered_it",
                "--skip", "a_frame_to_acknowledge_comes_from_the_last_render"]]),
    ("cargo-fuzz", [["cargo", "fuzz", "--version"]] + [
        ["cargo", "+" + a.nightly, "fuzz", "run", target, str(corpora / target), "--",
         "-max_total_time=" + str(a.fuzz_seconds), "-max_len=4096", "-rss_limit_mb=1024",
         "-timeout=5", "-seed=12345", "-artifact_prefix=" + str(out / "fuzz-artifacts" / target) + "/"] for target in FUZZ_TARGETS]),
    ("Kani", [["cargo", "kani", "--version"]] + kani_commands),
    ("Loom", [["cargo", "test", "--locked", "--manifest-path", manifest,
               "schedules::shared_immutable_buffer_independent_cursors", "--", "--exact"]]),
]
for name, commands in stages:
    stage = {"name": name, "status": "not_run", "commands": commands}
    report["stages"].append(stage)
save()
for stage in report["stages"]:
    if stage["name"] == "Kani" and not (a.kani_full or a.kani_harness):
        # Neither PASS nor N/A: this step simply belongs to release validation.
        # 既不是 PASS 也不是 N/A：这一步属于发版验证。
        stage.update(status="deferred_to_release", commands=[], record=report["kani_scope"]["record"])
        save()
        print("Kani: deferred to release", flush=True)
        continue
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
        for target in FUZZ_TARGETS:
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
    if stage["name"] == "Kani" and not a.kani_full:
        # One result per named harness; a limit is recorded and the chain continues, a counterexample stops it.
        # 每个指定 harness 单独记结果；撞上限登记后继续，反例则停止。
        stage["harness_results"] = {}
        harness_logs = out / "kani-harness-logs"
        harness_logs.mkdir()
        with (out / "Kani.log").open("w") as log:
            subprocess.run(["cargo", "kani", "--version"], stdout=log, stderr=subprocess.STDOUT, env=environment, check=False)
        for harness, command in zip(a.kani_harness, kani_commands):
            harness_log = harness_logs / (harness.replace(":", "_") + ".log")
            try:
                done = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, errors="replace", timeout=10800, env=environment)
                harness_log.write_text(done.stdout)
                status, reason = kani_policy.classify(done.returncode, done.stdout)
            except (OSError, subprocess.TimeoutExpired) as exc:
                status, reason = "failed", str(exc)
            stage["harness_results"][harness] = {"status": status, "reason": reason, "command": command}
            if harness_log.is_file():
                stage["harness_results"][harness]["log_sha256"] = hashlib.sha256(harness_log.read_bytes()).hexdigest()
            save()
            print("Kani " + harness + ": " + status + " (" + reason + ")", flush=True)
            if status == "failed":
                stage.update(status="failed", reason=harness + ": " + reason)
                save()
                sys.exit(1)
        undetermined = [name for name, result in stage["harness_results"].items() if result["status"] == "undetermined"]
        report["acceptance_pending"].extend("Kani undetermined: " + name + " (" + stage["harness_results"][name]["reason"] + ")" for name in undetermined)
        if any(result["status"] == "passed" for result in stage["harness_results"].values()):
            generated = list(Path(stage["environment"]["CARGO_TARGET_DIR"]).glob("**/out/production.rs"))
            if not generated or any(hashlib.sha256(path.read_bytes()).hexdigest() != report["production_regions_sha256"] for path in generated):
                stage.update(status="failed", reason="generated Kani production body mismatch")
                save()
                sys.exit(1)
            stage["generated_production_sha256"] = report["production_regions_sha256"]
        stage["log_sha256"] = hashlib.sha256((out / "Kani.log").read_bytes()).hexdigest()
        stage["finished_at"] = datetime.now(timezone.utc).isoformat()
        stage["status"] = "undetermined" if undetermined else "passed_scoped_checks"
        save()
        print("Kani: " + stage["status"], flush=True)
        continue
    with (out / (stage["name"].replace("+", "-") + ".log")).open("w") as log:
        try:
            for command in stage["commands"]:
                log.write(json.dumps(command) + "\n")
                log.flush()
                result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=10800, env=environment)
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
        "Proptest": [("test result: ok. 2 passed", 3), ("test result: ok. 3 passed", 1), ("test result: ok. 60 passed", 1), ("test result: ok. 8 passed", 1), ("test result: ok. 16 passed", 1)],
        "Miri": [("test result: ok. 2 passed", 4), ("test result: ok. 3 passed", 2), ("test result: ok. 6 passed", 1), ("test result: ok. 11 passed", 1)],
        "Kani": [("VERIFICATION:- SUCCESSFUL", 70)],
        "Loom": [("test result: ok. 1 passed", 1)],
    }
    matched = all(output.count(token) >= count for token, count in required.get(stage["name"], []))
    if stage["name"] == "cargo-fuzz":
        stage["completed_runs"] = [int(n) for n in re.findall(r"Done (\d+) runs in", output)]
        matched = len(stage["completed_runs"]) == len(FUZZ_TARGETS) and all(n > 0 for n in stage["completed_runs"])
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
report["scoped_checks_complete"] = all(stage["status"] in {"passed_scoped_checks", "deferred_to_release"} for stage in report["stages"])
report["slice_complete"] = not report["acceptance_pending"]
report["finished_at"] = datetime.now(timezone.utc).isoformat()
# Deliberately no whole-engine completion claim from this initial slice.
# 初始切片不能产生整个引擎验收完成结论。
save()

if report["acceptance_pending"]:
    print("Acceptance incomplete: " + "; ".join(report["acceptance_pending"]), flush=True)
    sys.exit(2)
