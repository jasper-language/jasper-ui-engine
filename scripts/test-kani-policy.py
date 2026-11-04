#!/usr/bin/env python3
"""Unit checks for the targeted Kani limits and result classification.
定点 Kani 上限与结果分类的单元检查。
"""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kani_policy

ONE_PASS = "Checking harness a...\nVERIFICATION:- SUCCESSFUL\n"


class KaniPolicy(unittest.TestCase):
    def test_command_names_the_harness_and_keeps_limits(self):
        command = kani_policy.command("m/Cargo.toml", "kani_checks::one")
        self.assertEqual(command[:2], ["flock", "/tmp/jasper-kani.lock"])
        self.assertEqual(command[-2:], ["--harness", "kani_checks::one"])
        self.assertIn("3500000", command)
        self.assertIn("600", command)

    def test_limits_cannot_be_raised(self):
        with self.assertRaises(ValueError):
            kani_policy.command("m", "a", seconds=601)
        with self.assertRaises(ValueError):
            kani_policy.command("m", "a", vmem_kb=3500001)
        self.assertIn("1", kani_policy.command("m", "a", seconds=1))

    def test_harness_name_is_never_shell_text(self):
        for name in ["", "a b", "a;b", "$(x)", "-x", "a::"]:
            with self.assertRaises(ValueError):
                kani_policy.command("m", name)

    def test_exactly_one_success_passes(self):
        self.assertEqual(kani_policy.classify(0, ONE_PASS)[0], "passed")

    def test_vacuous_or_widened_selection_fails(self):
        self.assertEqual(kani_policy.classify(0, "")[0], "failed")
        self.assertEqual(kani_policy.classify(0, ONE_PASS + ONE_PASS)[0], "failed")
        self.assertEqual(kani_policy.classify(0, "VERIFICATION:- SUCCESSFUL\n")[0], "failed")

    def test_counterexample_blocks(self):
        self.assertEqual(kani_policy.classify(1, "Failed Checks: x\nVERIFICATION:- FAILED\n"), ("failed", "counterexample"))
        self.assertEqual(kani_policy.classify(1, "VERIFICATION:- FAILED\n")[0], "failed")
        self.assertEqual(kani_policy.classify(0, ONE_PASS + "VERIFICATION:- FAILED\n")[0], "failed")

    def test_limits_are_undetermined_never_passed(self):
        self.assertEqual(kani_policy.classify(124, ""), ("undetermined", "time limit"))
        self.assertEqual(kani_policy.classify(124, ONE_PASS), ("undetermined", "time limit"))
        self.assertEqual(kani_policy.classify(1, "CBMC appears to have run out of memory.\nVERIFICATION:- FAILED\n"),
                         ("undetermined", "memory limit"))

    def test_other_errors_fail(self):
        self.assertEqual(kani_policy.classify(101, "error: no harnesses matched")[0], "failed")


if __name__ == "__main__":
    unittest.main()
