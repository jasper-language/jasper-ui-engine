#!/usr/bin/env python3
"""Audit the Jasper UI closeout inputs without claiming checklist completion.

The default mode always writes an audit report and exits zero. Use
``--require-ready`` only for the final delivery gate: it exits non-zero unless
items 1-5 have explicit completion records, the item branches are integrated,
evidence still matches its recorded sources, tool pins are consistent, and the
VM baseline regression passed.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
FINAL_AUDIT = "evidence/final-checklist-audit/report.json"
PROOF_BACKLOG_DOC = "docs/claude-closeout-proof-backlog.md"
DEFAULT_OUTPUT = "evidence/final-closeout-audit"

EXPECTED = {
    "rust": "1.99.1",
    "nightly": "nightly-2026-10-17",
    "verus": "0.2026.10.18.7d82994",
    "verus_sha256": "e9af03732d4bdd32b93a8fcf471706739c24c1473fa5086618d96c47237845dd",
    "z3": "4.16.0",
    "kani": "0.68.0",
    "cbmc": "6.11.0",
    "cargo_fuzz": "0.13.2",
    "vm_default": {"memory": "3G", "swap": "0G", "seconds": 180, "tasks": 128, "jobs": 1},
}

CHECKLIST = {
    1: {
        "title": "公共契约、场景/组件更新与真实 Map 证明",
        "source_paths": [
            "src/scene.rs",
            "src/batch.rs",
            "src/input_identity.rs",
            "src/engine.rs",
            "src/native_adapter/native_scene.rs",
            "src/native_adapter/native_view.rs",
            "src/native_adapter/component_contract.rs",
            "src/native_adapter/update_contract.rs",
            "src/native_adapter/render_host.rs",
            "tests/native_frame_transaction.rs",
            "tests/native_update_contract.rs",
            "tests/native_component_update_contract.rs",
            "verification/read-boundary/src/lib.rs",
        ],
        "evidence_paths": [
            FINAL_AUDIT,
        ],
    },
    2: {
        "title": "Engine/Blitz 整帧事务、默认入口与呈现",
        "source_paths": [
            "src/engine.rs",
            "src/backend.rs",
            "src/native_adapter/native_scene.rs",
            "src/native_adapter/native_view.rs",
            "src/native_adapter/frame_schedule.rs",
            "src/native_adapter/facade.rs",
            "src/native_adapter/render_host.rs",
            "tests/native_frame_transaction.rs",
            "tests/native_frame_sequences.rs",
            "tests/native_frame_differential.rs",
            "tests/native_cpu_acceptance.rs",
            "tests/native_frame_damage.rs",
            "tests/native_frame_damage_write.rs",
            "tests/native_frame_schedule.rs",
            "tests/native_frame_pace.rs",
        ],
        "evidence_paths": [
            FINAL_AUDIT,
        ],
    },
    3: {
        "title": "渲染、字体、宿主 Surface 与 GPU 生命周期",
        "source_paths": [
            "src/native_adapter/gpu_render.rs",
            "src/native_adapter/native_fonts.rs",
            "src/native_adapter/glyph_skew.rs",
            "src/native_adapter/text_geometry.rs",
            "src/native_adapter/native_scene.rs",
            "src/native_adapter/facade.rs",
            "src/native_adapter/render_host.rs",
            "src/surface_lifecycle.rs",
            "src/host_capabilities.rs",
            "examples/gpu_window.rs",
            "examples/window.rs",
            "tests/native_host_renderer.rs",
            "tests/native_gpu_host_view.rs",
            "tests/native_gpu_render.rs",
            "tests/native_gpu_frame_damage.rs",
            "tests/native_font_synthesis.rs",
            "tests/native_font_family.rs",
            "tests/native_font_weight_style.rs",
        ],
        "evidence_paths": [
            FINAL_AUDIT,
        ],
    },
    4: {
        "title": "交互、外部输入/组合服务与无障碍宿主",
        "source_paths": [
            "src/input.rs",
            "src/input_identity.rs",
            "src/pointer.rs",
            "src/keyboard.rs",
            "src/composition_identity.rs",
            "src/resource.rs",
            "src/desktop.rs",
            "src/native_adapter/access_kit.rs",
            "src/native_adapter/access_tree.rs",
            "src/native_adapter/capture_policy.rs",
            "src/native_adapter/native_events.rs",
            "src/native_adapter/native_keys.rs",
            "src/native_adapter/pointer_host.rs",
            "src/native_adapter/scene_events.rs",
            "src/native_adapter/facade.rs",
            "tests/native_interaction_closure.rs",
            "tests/input_admission.rs",
            "tests/resource_gate.rs",
            "tests/native_accessibility_tree.rs",
            "tests/native_accessibility_update.rs",
            "tests/native_accesskit_bridge.rs",
        ],
        "evidence_paths": [
            FINAL_AUDIT,
        ],
    },
    5: {
        "title": "公共 API 原型迁移、完整示例与性能基线",
        "source_paths": [
            "src/lib.rs",
            "src/engine.rs",
            "src/backend.rs",
            "examples/full_scene.rs",
            "examples/hello.rs",
            "examples/pointer.rs",
            "examples/window.rs",
            "src/native_adapter/render_host.rs",
            "tests/native_item5_journey.rs",
            "tests/native_cpu_acceptance.rs",
            "tests/native_blitz_engine.rs",
            "tests/native_frame_differential.rs",
        ],
        "evidence_paths": [
            FINAL_AUDIT,
            "evidence/item5-full-path-baseline/report.json",
        ],
    },
}

PRIOR_BRANCHES = {
    "codex/item2-engine": "item 2",
    "codex/item3-render": "item 3",
    "codex/item4-interaction": "item 4",
    "codex/item5-migration": "item 5",
}

PROOF_BACKLOG = {
    "CP-01": {
        "title": "真实捕获存储任意双键 Kani 义务",
        "evidence": [
            "docs/status.md",
            FINAL_AUDIT,
        ],
    },
    "CP-02": {
        "title": "真实 BTreeMap source_subtree_contains 与整帧实现级证明",
        "evidence": [
            "evidence/native-subtree-solver-capability/report.json",
            "evidence/native-subtree-expanded-capability/report.json",
            "evidence/scene-frame-completion-chain/report.json",
        ],
    },
    "CP-03": {
        "title": "NativeScene 候选构造、状态恢复与整帧有序证明",
        "evidence": [
            "evidence/native-scene-candidate-transaction-pending/report.json",
            "evidence/scene-frame-completion-kernel/report.json",
            "evidence/scene-frame-order-kernel/report.json",
            "evidence/scene-frame-order-chain/report.json",
            "evidence/scene-batch-stage-kernel/report.json",
            "evidence/scene-batch-stage-chain/report.json",
            "evidence/scene-interaction-restore-kernel/report.json",
            "evidence/scene-interaction-restore-chain/report.json",
            "evidence/scene-node-remove-kernel/report.json",
            "evidence/scene-node-remove-chain/report.json",
            "evidence/scene-node-find-kernel/report.json",
            "evidence/scene-node-find-chain/report.json",
            "evidence/scene-patch-kernel/report.json",
            "evidence/scene-patch-chain/report.json",
            "evidence/scene-unique-ids-kernel/report.json",
            "evidence/scene-unique-ids-chain/report.json",
            "evidence/scene-batch-commit-kernel/report.json",
            "evidence/scene-batch-commit-chain/report.json",
            "evidence/scene-checked-patch-kernel/report.json",
            "evidence/scene-checked-patch-chain/report.json",
            "evidence/scene-batch-unique-kernel/report.json",
            "evidence/scene-batch-unique-chain/report.json",
        ],
    },
    "CP-04": {
        "title": "浮点、布局、颜色混合及一般原生化变更规格",
        "evidence": [
            "docs/verification.md",
            FINAL_AUDIT,
        ],
    },
    "CP-05": {
        "title": "DesktopInput/Instant 状态的 Kani 时钟边界",
        "evidence": [
            "docs/input-method-boundary.md",
            "docs/status.md",
        ],
    },
    "CP-06": {
        "title": "宿主 GPU Surface/device-loss/vsync 六阶段与能力矩阵",
        "evidence": [
            "evidence/native-gpu-host-target-pending/report.json",
            "evidence/window-xvfb-presentation/report.json",
            "evidence/surface-lifecycle-kernel/report.json",
            "evidence/surface-lifecycle-chain/report.json",
            "evidence/host-surface-coordination-kernel/report.json",
            "evidence/host-surface-coordination-chain/report.json",
            "evidence/host-render-outcome-kernel/report.json",
            "evidence/host-render-outcome-chain/report.json",
            "evidence/capability-matrix-kernel/report.json",
            "evidence/capability-matrix-chain/report.json",
            "evidence/render-tcb/report.json",
        ],
    },
    "CP-07": {
        "title": "活外部 IME/无障碍服务与组合生命周期证明",
        "evidence": [
            "docs/input-method-boundary.md",
            "docs/api.md",
            FINAL_AUDIT,
        ],
    },
    "CP-08": {
        "title": "full_scene/窗口适配器实现级组合证明与跨宿主性能门",
        "evidence": [
            "evidence/item5-full-path-baseline/report.json",
            "docs/claude-item5-proof-backlog.md",
            "examples/full_scene.rs",
            FINAL_AUDIT,
        ],
    },
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def run_git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=check,
    )


def repo_path(value: str, base: Path | None = None) -> Path:
    candidate = Path(value)
    if not candidate.is_absolute():
        candidate = (base or ROOT) / candidate
    return candidate.resolve()


def rel(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path.resolve())


def source_records(report_path: Path, data: dict[str, Any]) -> list[dict[str, Any]]:
    mappings: list[tuple[str, dict[str, Any]]] = []
    for key in ("sources", "source"):
        value = data.get(key)
        if isinstance(value, dict):
            mappings.append((key, value))
    records: list[dict[str, Any]] = []
    for field, mapping in mappings:
        for source, expected in sorted(mapping.items()):
            if not isinstance(expected, str):
                continue
            path = repo_path(source)
            record: dict[str, Any] = {
                "field": field,
                "path": rel(path),
                "exists": path.is_file(),
                "expected_sha256": expected,
            }
            if path.is_file():
                record["actual_sha256"] = sha256(path)
                record["status"] = "current" if record["actual_sha256"] == expected else "stale"
            else:
                record["status"] = "missing"
            record["report"] = rel(report_path)
            records.append(record)
    return records


def evidence_record(path: Path) -> dict[str, Any]:
    record: dict[str, Any] = {"path": rel(path), "exists": path.is_file()}
    if not path.is_file():
        record.update(status="missing", sources=[], completion_flags={})
        return record
    try:
        data = read_json(path)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        record.update(status="invalid_json", error=str(exc), sources=[], completion_flags={})
        return record
    flags = {
        key: value
        for key, value in data.items()
        if re.fullmatch(r"(?:whole_)?checklist_item_(?:[1-6]_)?complete", key)
    }
    record.update(
        status=data.get("status", "unspecified"),
        scope=data.get("scope"),
        sources=source_records(path, data),
        completion_flags=flags,
    )
    return record


def evidence_index() -> list[dict[str, Any]]:
    index = []
    for path in sorted((ROOT / "evidence").glob("*/report.json")):
        record = evidence_record(path)
        sources = record.get("sources", [])
        stale = [source["path"] for source in sources if source["status"] != "current"]
        index.append(
            {
                "path": record["path"],
                "sha256": sha256(path),
                "status": record["status"],
                "recorded_sources": len(sources),
                "stale_or_missing_sources": stale,
                "completion_flags": record.get("completion_flags", {}),
            }
        )
    return index


def final_report_evidence_paths(final_report: dict[str, Any]) -> list[Path]:
    base = (ROOT / FINAL_AUDIT).parent
    paths: set[Path] = set()
    for item in final_report.get("items", {}).values():
        if not isinstance(item, dict):
            continue
        for value in item.get("evidence", []):
            if isinstance(value, str):
                paths.add(repo_path(value, base))
    return sorted(paths)


def item_completion_state(
    number: int,
    final_item: dict[str, Any],
    evidence: list[dict[str, Any]],
) -> dict[str, Any]:
    final_status = final_item.get("status", "missing")
    flags: dict[str, bool | None] = {}
    for record in evidence:
        for key, value in record.get("completion_flags", {}).items():
            if re.search(rf"(?:^|_){number}(?:_|$)", key):
                flags[f"{record['path']}:{key}"] = value if isinstance(value, bool) else None
    blockers = [value for value in final_item.get("blocking_gaps", []) if isinstance(value, str)]
    remaining: list[str] = []
    if evidence:
        data_paths = {record["path"] for record in evidence if record.get("exists")}
        if FINAL_AUDIT in data_paths:
            try:
                final = read_json(ROOT / FINAL_AUDIT)
                key = next(
                    (
                        key
                        for key in final.get("items", {})
                        if key.startswith(f"{number}_")
                    ),
                    None,
                )
                if key:
                    remaining.extend(final["items"][key].get("blocking_gaps", []))
            except (OSError, ValueError, json.JSONDecodeError):
                pass
    current_prefix = FINAL_AUDIT + ":"
    current_flags = {
        key: value for key, value in flags.items() if key.startswith(current_prefix)
    }
    historical_flags = {
        key: value for key, value in flags.items() if not key.startswith(current_prefix)
    }
    explicit_current_true = bool(current_flags) and all(
        value is True for value in current_flags.values()
    )
    completion_scope = final_item.get("completion_standard")
    scope_is_current = completion_scope == "functional_gaps_closed_and_basic_proofs_passed"
    complete = (
        final_status == "complete"
        and explicit_current_true
        and scope_is_current
        and not blockers
        and not remaining
    )
    if complete:
        reading = "explicit_complete_current_scope"
    elif (
        any(value is False for value in current_flags.values())
        or final_status != "complete"
        or not scope_is_current
        or blockers
        or remaining
    ):
        reading = "not_complete"
    else:
        reading = "not_claimed"
    return {
        "recorded_status": final_status,
        "completion_reading": reading,
        "complete": complete,
        "completion_standard": completion_scope,
        "evidence_completion_flags": flags,
        "superseded_historical_completion_flags": {
            key: value for key, value in historical_flags.items() if value is False
        },
        "blocking_gaps": blockers,
        "remaining": remaining,
    }


def tool_config_audit() -> dict[str, Any]:
    checks = []

    def require_text(identifier: str, path: str, needles: list[str]) -> None:
        file = ROOT / path
        text = file.read_text() if file.is_file() else ""
        missing = [needle for needle in needles if needle not in text]
        checks.append(
            {
                "id": identifier,
                "path": path,
                "status": "passed" if file.is_file() and not missing else "failed",
                "missing": missing if file.is_file() else ["file"],
            }
        )

    require_text(
        "ci-tool-pins",
        ".github/workflows/ci.yml",
        [
            EXPECTED["rust"],
            EXPECTED["nightly"],
            EXPECTED["verus"],
            EXPECTED["verus_sha256"],
            EXPECTED["cargo_fuzz"],
        ],
    )
    require_text(
        "vm-default-limits",
        "scripts/verify-in-vm.sh",
        [
            '${JASPER_UI_VM_TIMEOUT_SECONDS:-180}',
            '${JASPER_UI_VM_MEMORY:-3G}',
            '${JASPER_UI_VM_SWAP:-0G}',
            "--property=TasksMax=128",
            "CARGO_BUILD_JOBS=1",
        ],
    )
    require_text(
        "ordered-chain-pending-gate",
        "scripts/verify-ui.py",
        [
            '"CP-01: original arbitrary two-key Kani obligation is unresolved; single-key harness is diagnostic only"',
            '"CP-02: real BTreeMap Kani / Bitwuzla path is unresolved"',
            '"slice_complete": False',
            '"complete": False',
        ],
    )

    version_conflicts: list[dict[str, str]] = []
    patterns = {
        "verus": r"Verus\s+(?:Version:\s*)?([0-9]{4}\.[0-9]{2}\.[0-9]{2}\.[0-9a-f]+)",
        "z3": r"Z3(?:\s+version)?\s+([0-9]+\.[0-9]+\.[0-9]+)",
        "kani": r"Kani\s+([0-9]+\.[0-9]+\.[0-9]+)",
        "cbmc": r"CBMC\s+([0-9]+\.[0-9]+\.[0-9]+)",
        "nightly": r"(nightly-[0-9]{4}-[0-9]{2}-[0-9]{2})",
    }
    for path in sorted((ROOT / "evidence").glob("*/report.json")):
        try:
            text = path.read_text()
        except OSError:
            continue
        for tool, pattern in patterns.items():
            for found in sorted(set(re.findall(pattern, text))):
                if found != EXPECTED[tool]:
                    version_conflicts.append(
                        {"tool": tool, "found": found, "expected": EXPECTED[tool], "path": rel(path)}
                    )
    return {
        "expected": EXPECTED,
        "static_checks": checks,
        "evidence_version_conflicts": version_conflicts,
        "status": "passed"
        if all(check["status"] == "passed" for check in checks) and not version_conflicts
        else "failed",
    }


def integration_audit() -> dict[str, Any]:
    branches = []
    head = run_git("rev-parse", "HEAD").stdout.strip()
    for branch, label in PRIOR_BRANCHES.items():
        exists = run_git("rev-parse", "--verify", "--quiet", branch, check=False).returncode == 0
        if not exists:
            branches.append(
                {
                    "branch": branch,
                    "item": label,
                    "exists": False,
                    "contained": False,
                    "ahead": None,
                    "tip": None,
                }
            )
            continue
        tip = run_git("rev-parse", branch).stdout.strip()
        contained = (
            run_git("merge-base", "--is-ancestor", branch, "HEAD", check=False).returncode == 0
        )
        ahead = int(
            run_git("rev-list", "--count", f"HEAD..{branch}", check=False).stdout.strip() or 0
        )
        branches.append(
            {
                "branch": branch,
                "item": label,
                "exists": True,
                "contained": contained,
                "ahead": ahead,
                "tip": tip,
            }
        )
    return {
        "branches": branches,
        "status": "included"
        if branches and all(branch["exists"] and branch["contained"] for branch in branches)
        else "pending",
        "same_tip_branches": [
            branch["branch"] for branch in branches if branch.get("tip") == head
        ],
        "interpretation": (
            "Ancestry only verifies that the branch refs are included; item completion "
            "must still be established by explicit checklist evidence."
        ),
    }


def source_dirty_paths(lines: list[str], output: Path) -> list[str]:
    output_prefix = rel(output) + "/"
    return [
        line
        for line in lines
        if line.strip() and output_prefix not in line
    ]


def proof_backlog_audit() -> dict[str, Any]:
    doc = ROOT / PROOF_BACKLOG_DOC
    text = doc.read_text() if doc.is_file() else ""
    items = []
    for identifier, definition in PROOF_BACKLOG.items():
        evidence = []
        for value in definition["evidence"]:
            path = repo_path(value)
            evidence.append({"path": rel(path), "exists": path.is_file()})
        items.append(
            {
                "id": identifier,
                "title": definition["title"],
                "documented": f"### {identifier}" in text,
                "evidence": evidence,
            }
        )
    missing_evidence = [
        evidence["path"]
        for item in items
        for evidence in item["evidence"]
        if not evidence["exists"]
    ]
    return {
        "document": PROOF_BACKLOG_DOC,
        "items": items,
        "missing_evidence": sorted(set(missing_evidence)),
        "status": "passed"
        if items and all(item["documented"] for item in items) and not missing_evidence
        else "incomplete",
        "completion_claimed": False,
    }


def static_audit(output: Path | None = None) -> dict[str, Any]:
    final_path = ROOT / FINAL_AUDIT
    final: dict[str, Any] = {}
    final_error = None
    try:
        final = read_json(final_path)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        final_error = str(exc)

    final_keys = {
        int(key.split("_", 1)[0]): value
        for key, value in final.get("items", {}).items()
        if isinstance(value, dict) and re.match(r"^[1-6]_", key)
    }
    items: dict[str, Any] = {}
    missing_required_sources: list[str] = []
    missing_required_evidence: list[str] = []
    for number, definition in CHECKLIST.items():
        sources = []
        for value in definition["source_paths"]:
            path = repo_path(value)
            exists = path.is_file()
            sources.append(
                {
                    "path": rel(path),
                    "exists": exists,
                    "sha256": sha256(path) if exists else None,
                }
            )
            if not exists:
                missing_required_sources.append(rel(path))
        evidence = []
        for value in definition["evidence_paths"]:
            path = repo_path(value)
            record = evidence_record(path)
            evidence.append(record)
            if not record["exists"]:
                missing_required_evidence.append(record["path"])
        completion = item_completion_state(number, final_keys.get(number, {}), evidence)
        stale_sources = [
            source
            for record in evidence
            for source in record.get("sources", [])
            if source["status"] != "current"
        ]
        items[str(number)] = {
            "title": definition["title"],
            **completion,
            "sources": sources,
            "evidence": evidence,
            "source_correspondence": {
                "status": "current" if not stale_sources else "stale_or_missing",
                "issues": stale_sources,
            },
        }

    final_references = []
    missing_final_references = []
    for path in final_report_evidence_paths(final):
        key = rel(path)
        exists = path.is_file()
        final_references.append({"path": key, "exists": exists})
        if not exists:
            missing_final_references.append(key)

    git_branch = run_git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    git_commit = run_git("rev-parse", "HEAD").stdout.strip()
    status_lines = run_git("status", "--porcelain").stdout.splitlines()
    dirty = source_dirty_paths(status_lines, output or ROOT / DEFAULT_OUTPUT)
    recorded_commits = final.get("commits", [])
    recorded_commit_shas = [
        value.split()[0]
        for value in recorded_commits
        if isinstance(value, str) and re.match(r"^[0-9a-f]{7,40}\b", value)
    ]
    final_report_basis = None
    for candidate in reversed(recorded_commit_shas):
        if run_git("merge-base", "--is-ancestor", candidate, "HEAD", check=False).returncode == 0:
            final_report_basis = candidate
            break
    material_gap: list[str] = []
    if final_report_basis:
        material_gap = [
            path
            for path in run_git("diff", "--name-only", f"{final_report_basis}..HEAD", check=False)
            .stdout.splitlines()
            if path
            and (
                path.startswith(("src/", "tests/", "examples/", "verification/", "fuzz/", "scripts/"))
                or path in {"Cargo.toml", "Cargo.lock", "docs/status.md", "docs/verification.md"}
            )
        ]
    final_audit_covers_head = final_report_basis is not None and not material_gap

    completed = sum(1 for item in items.values() if item["complete"])
    reports = evidence_index()
    blockers = [
        gap
        for number in range(1, 6)
        for gap in items[str(number)]["blocking_gaps"]
    ]
    correspondence_issues = [
        issue
        for item in items.values()
        for issue in item["source_correspondence"]["issues"]
    ]
    summary = {
        "checklist_items": 6,
        "items_1_to_5_with_explicit_completion": completed,
        "completion_claimed": False,
        "static_audit_complete": final_error is None,
        "final_report_covers_head": final_audit_covers_head,
        "final_report_basis": final_report_basis,
        "material_paths_changed_since_final_report_basis": len(material_gap),
        "missing_required_sources": len(set(missing_required_sources)),
        "missing_required_evidence": len(set(missing_required_evidence)),
        "missing_final_report_references": len(set(missing_final_references)),
        "stale_or_missing_source_correspondence": len(correspondence_issues),
        "unresolved_item_blockers": len(blockers),
    }
    return {
        "schema_version": 1,
        "kind": "jasper-ui-closeout-audit",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "claim": "audit only; no checklist completion or whole-engine acceptance is claimed",
        "git": {
            "branch": git_branch,
            "commit": git_commit,
            "dirty_paths": dirty,
        },
        "final_audit": {
            "path": FINAL_AUDIT,
            "readable": final_error is None,
            "error": final_error,
            "recorded_status": final.get("status"),
            "recorded_completed_items": final.get("completed_items"),
            "covers_head": final_audit_covers_head,
            "basis_commit": final_report_basis,
            "material_changed_since_basis": material_gap,
            "references": final_references,
            "missing_references": missing_final_references,
        },
        "audit_inputs": {
            path: sha256(ROOT / path)
            for path in [
                "scripts/audit-closeout.py",
                "scripts/closeout-baseline.sh",
                "scripts/test-audit-closeout.py",
                PROOF_BACKLOG_DOC,
                "docs/closeout.md",
                "scripts/verify-ui.py",
                "scripts/verify-in-vm.sh",
                ".github/workflows/ci.yml",
                "Cargo.toml",
                "Cargo.lock",
            ]
            if (ROOT / path).is_file()
        },
        "evidence_index": reports,
        "evidence_index_summary": {
            "reports": len(reports),
            "with_stale_or_missing_recorded_sources": sum(
                bool(report["stale_or_missing_sources"]) for report in reports
            ),
            "with_explicit_completion": sum(
                bool(report["completion_flags"]) for report in reports
            ),
        },
        "checklist": {
            "definition": "Items 1-6 from the 2026-11-02 total audit; only explicit completion records count.",
            "items": items,
        },
        "tool_configuration": tool_config_audit(),
        "branch_integration": integration_audit(),
        "complex_proof_backlog": proof_backlog_audit(),
        "summary": summary,
    }


def render_summary(report: dict[str, Any]) -> str:
    summary = report["summary"]
    git = report["git"]
    lines = [
        "# Jasper UI closeout audit",
        "",
        "This report is an audit snapshot, not a delivery acceptance record.",
        "",
        f"- branch: `{git['branch']}`",
        f"- commit: `{git['commit']}`",
        f"- explicit item 1-5 completions: {summary['items_1_to_5_with_explicit_completion']}/5",
        f"- final report materially covers HEAD: `{str(summary['final_report_covers_head']).lower()}`",
        f"- missing required evidence: {summary['missing_required_evidence']}",
        f"- missing final-report references: {summary['missing_final_report_references']}",
        f"- stale/missing recorded source hashes: {summary['stale_or_missing_source_correspondence']}",
        f"- branch refs included: `{report['branch_integration']['status']}`",
        f"- tool configuration: `{report['tool_configuration']['status']}`",
        f"- evidence reports indexed: {report['evidence_index_summary']['reports']}",
        "",
        "## Checklist readings",
        "",
        "| Item | Reading |",
        "|---:|---|",
    ]
    for number, item in report["checklist"]["items"].items():
        lines.append(f"| {number} | `{item['completion_reading']}` |")
    lines.extend(["", "## Missing final-audit evidence", ""])
    missing = report["final_audit"]["missing_references"]
    if missing:
        lines.extend(f"- `{path}`" for path in missing)
    else:
        lines.append("None.")
    issues = []
    seen = set()
    for item in report["checklist"]["items"].values():
        for issue in item["source_correspondence"]["issues"]:
            key = (issue["report"], issue["path"])
            if key not in seen:
                issues.append(issue)
                seen.add(key)
    lines.extend(["", "## Stale or missing source correspondence", ""])
    if issues:
        lines.extend(
            f"- `{issue['path']}` in `{issue['report']}`: `{issue['status']}`"
            for issue in issues
        )
    else:
        lines.append("None.")
    lines.extend(
        [
            "",
            "Run `python3 scripts/audit-closeout.py --require-ready` only after items 2-5",
            "have been integrated and their evidence has been regenerated.",
            "",
        ]
    )
    return "\n".join(lines)


def run_baseline(output: Path) -> dict[str, Any]:
    log = output / "baseline.log"
    command = [
        str(ROOT / "scripts/verify-in-vm.sh"),
        "env",
    ]
    for name in (
        "JASPER_UI_TEST_FONT",
        "JASPER_UI_TEST_LATIN_FONT",
        "JASPER_UI_TEST_CJK_FONT",
        "JASPER_UI_TEST_BOLD_FONT",
        "JASPER_UI_TEST_ITALIC_FONT",
        "CLOSEOUT_RUN_NATIVE_INTEGRATION",
        "CLOSEOUT_RUN_GPU",
        "CARGO_PROFILE_DEV_DEBUG",
        "CARGO_PROFILE_TEST_DEBUG",
    ):
        if value := os.environ.get(name):
            command.append(f"{name}={value}")
    command += [
        "bash",
        "scripts/closeout-baseline.sh",
    ]
    started = datetime.now(timezone.utc).isoformat()
    try:
        result = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=1900,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        log.write_text(f"$ {' '.join(command)}\n{exc}\n")
        return {
            "status": "unavailable",
            "started_at": started,
            "finished_at": datetime.now(timezone.utc).isoformat(),
            "command": command,
            "log": rel(log),
            "error": str(exc),
        }
    log.write_text(f"$ {' '.join(command)}\n{result.stdout}")
    status = "passed" if result.returncode == 0 else "failed"
    if result.returncode == 2 and "Set JASPER_UI_VM_RUN" in result.stdout:
        status = "unavailable"
    return {
        "status": status,
        "started_at": started,
        "finished_at": datetime.now(timezone.utc).isoformat(),
        "command": command,
        "returncode": result.returncode,
        "log": rel(log),
        "log_sha256": sha256(log),
    }


def final_gate_failures(report: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    summary = report["summary"]
    if report["final_audit"]["missing_references"]:
        failures.append("final audit has missing evidence references")
    if summary["items_1_to_5_with_explicit_completion"] != 5:
        failures.append("items 1-5 do not all have explicit completion records")
    if summary["unresolved_item_blockers"]:
        failures.append("items 1-5 still record blocking gaps")
    if summary["missing_required_evidence"]:
        failures.append("required evidence is missing")
    if summary["missing_required_sources"]:
        failures.append("required source files are missing")
    if summary["stale_or_missing_source_correspondence"]:
        failures.append("evidence source hashes are stale or missing")
    if not summary["final_report_covers_head"]:
        failures.append("final audit does not cover HEAD")
    if report["branch_integration"]["status"] != "included":
        failures.append("prior item branch refs are not included")
    if report["tool_configuration"]["status"] != "passed":
        failures.append("tool configuration audit failed")
    if report["complex_proof_backlog"]["status"] != "passed":
        failures.append("complex proof backlog document is incomplete")
    baseline = report.get("baseline_regression", {})
    if baseline.get("status") != "passed":
        failures.append("VM baseline regression did not pass")
    if report["git"]["dirty_paths"]:
        failures.append("worktree is dirty")
    return failures


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        default=DEFAULT_OUTPUT,
        help="repository-relative output directory (default: %(default)s)",
    )
    parser.add_argument(
        "--run-baseline",
        action="store_true",
        help="run checks through scripts/verify-in-vm.sh; requires JASPER_UI_VM_RUN",
    )
    parser.add_argument(
        "--require-ready",
        action="store_true",
        help="exit non-zero unless every final-delivery gate passes",
    )
    parser.add_argument(
        "--print-summary",
        action="store_true",
        help="print the human-readable summary after writing reports",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    output = repo_path(args.output)
    if ROOT not in output.parents and output != ROOT:
        raise SystemExit("output must stay inside the repository")
    output.mkdir(parents=True, exist_ok=True)

    baseline = {
        "status": "not_run",
        "reason": "use --run-baseline after work is integrated; heavy checks stay inside the VM",
    }
    if args.run_baseline:
        baseline = run_baseline(output)

    report = static_audit(output)
    report["baseline_regression"] = baseline
    failures = final_gate_failures(report)
    report["final_gate"] = {
        "ready": not failures,
        "failures": failures,
        "note": "Default audits may be incomplete; --require-ready enforces this exact list.",
    }
    report_path = output / "report.json"
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    summary_path = output / "summary.md"
    summary_path.write_text(render_summary(report))

    print(
        json.dumps(
            {
                "report": rel(report_path),
                "summary": rel(summary_path),
                "explicit_item_1_to_5_completions": report["summary"][
                    "items_1_to_5_with_explicit_completion"
                ],
                "final_gate_ready": report["final_gate"]["ready"],
            },
            ensure_ascii=False,
        )
    )
    if args.print_summary:
        print()
        print(summary_path.read_text(), end="")
    if args.require_ready and failures:
        for failure in failures:
            print("NOT READY: " + failure, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
