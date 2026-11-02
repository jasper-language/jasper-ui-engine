#!/usr/bin/env python3
"""Focused tests for the closeout audit's conservative evidence readings."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("audit-closeout.py")
SPEC = importlib.util.spec_from_file_location("audit_closeout", SCRIPT)
assert SPEC and SPEC.loader
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class EvidenceAuditTests(unittest.TestCase):
    def test_current_source_hash_is_accepted(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "source.rs"
            source.write_text("fn main() {}\n")
            report = root / "report.json"
            report.write_text(
                json.dumps(
                    {
                        "status": "scoped_ordered_acceptance_complete",
                        "sources": {"source.rs": hashlib.sha256(source.read_bytes()).hexdigest()},
                    }
                )
            )
            with mock.patch.object(audit, "ROOT", root):
                record = audit.evidence_record(report)
            self.assertEqual(record["sources"][0]["status"], "current")

    def test_stale_source_hash_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "source.rs"
            source.write_text("changed\n")
            report = root / "report.json"
            report.write_text(
                json.dumps({"status": "pending", "sources": {"source.rs": "0" * 64}})
            )
            with mock.patch.object(audit, "ROOT", root):
                record = audit.evidence_record(report)
            self.assertEqual(record["sources"][0]["status"], "stale")

    def test_missing_evidence_never_counts_as_valid(self):
        with tempfile.TemporaryDirectory() as tmp:
            record = audit.evidence_record(Path(tmp) / "missing.json")
            self.assertFalse(record["exists"])
            self.assertEqual(record["status"], "missing")
            self.assertEqual(record["sources"], [])

    def test_false_completion_flag_keeps_item_incomplete(self):
        final_item = {
            "status": "incomplete",
            "blocking_gaps": ["remaining proof"],
        }
        evidence = [
            {
                "path": "evidence/example/report.json",
                "completion_flags": {"checklist_item_2_complete": False},
            }
        ]
        state = audit.item_completion_state(2, final_item, evidence)
        self.assertEqual(state["completion_reading"], "not_complete")
        self.assertFalse(state["complete"])

    def test_current_scope_supersedes_historical_false_flag(self):
        final_item = {
            "status": "complete",
            "completion_standard": "functional_gaps_closed_and_basic_proofs_passed",
            "blocking_gaps": [],
        }
        evidence = [
            {
                "path": audit.FINAL_AUDIT,
                "completion_flags": {"checklist_item_2_complete": True},
            },
            {
                "path": "evidence/historical/report.json",
                "completion_flags": {"checklist_item_2_complete": False},
            },
        ]
        state = audit.item_completion_state(2, final_item, evidence)
        self.assertEqual(state["completion_reading"], "explicit_complete_current_scope")
        self.assertTrue(state["complete"])
        self.assertEqual(
            state["superseded_historical_completion_flags"],
            {"evidence/historical/report.json:checklist_item_2_complete": False},
        )

    def test_scoped_success_without_current_scope_standard_is_not_complete(self):
        final_item = {
            "status": "complete",
            "completion_standard": "old_whole_proof_standard",
            "blocking_gaps": [],
        }
        evidence = [
            {
                "path": audit.FINAL_AUDIT,
                "completion_flags": {"checklist_item_3_complete": True},
            }
        ]
        state = audit.item_completion_state(3, final_item, evidence)
        self.assertFalse(state["complete"])

    def test_final_gate_rejects_audit_without_baseline(self):
        report = {
            "summary": {
                "items_1_to_5_with_explicit_completion": 5,
                "unresolved_item_blockers": 0,
                "missing_required_evidence": 0,
                "missing_required_sources": 0,
                "stale_or_missing_source_correspondence": 0,
                "final_report_covers_head": True,
            },
            "final_audit": {"missing_references": []},
            "branch_integration": {"status": "included"},
            "tool_configuration": {"status": "passed"},
            "complex_proof_backlog": {"status": "passed"},
            "baseline_regression": {"status": "not_run"},
            "git": {"dirty_paths": []},
        }
        failures = audit.final_gate_failures(report)
        self.assertEqual(failures, ["VM baseline regression did not pass"])

    def test_generated_audit_directory_does_not_dirty_source_tree(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            output = root / "evidence" / "final-closeout-audit"
            lines = [
                " M src/lib.rs",
                " M evidence/final-closeout-audit/report.json",
                "?? docs/note.md",
            ]
            with mock.patch.object(audit, "ROOT", root):
                dirty = audit.source_dirty_paths(lines, output)
            self.assertEqual(dirty, [" M src/lib.rs", "?? docs/note.md"])


if __name__ == "__main__":
    unittest.main()
