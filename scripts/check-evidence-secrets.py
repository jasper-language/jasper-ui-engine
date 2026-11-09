#!/usr/bin/env python3
"""Reject personal identifiers in tracked text files before they reach a public repository.

The repository is public. Apple account material - certificate names with team identifiers,
device UDIDs, provisioning profile names and machine-local home paths - must never be committed,
including inside evidence logs and reports.

公共仓库不得包含 Apple 账户材料（带团队标识的证书名、设备 UDID、描述文件名）与本机主目录路径，
包括证据日志与报告。

Usage: python3 scripts/check-evidence-secrets.py [--all]
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PATTERNS = [
    # Modern device UDID (8-16 hex with a dash). The legacy 40-hex form is not matched because
    # it is indistinguishable from the git revisions this repository pins everywhere.
    # 新式设备 UDID；旧式 40 位十六进制与仓库里大量固定的 git revision 无法区分，不纳入匹配。
    ("device_udid", re.compile(r"\b[0-9A-Fa-f]{8}-[0-9A-Fa-f]{16}\b")),
    # Certificate name carrying the team identifier, e.g. Apple Development: Jane (AB12CD34EF).
    ("development_certificate", re.compile(r"Apple Development:\s*[^(\n]+\([A-Z0-9]{10}\)")),
    # Machine-local home paths (the evidence sanitizer uses <home>/<repo> placeholders).
    ("home_path", re.compile(r"/Users/[A-Za-z0-9._-]+")),
]

# Placeholders and documentation that intentionally describe the shape of a value.
ALLOW = [
    re.compile(r"Apple Development:\s*\.\.\."),
    re.compile(r"<device-id>"),
    re.compile(r"<home>"),
    re.compile(r"<repo>"),
]

TEXT_SUFFIXES = {
    ".md", ".rs", ".toml", ".sh", ".py", ".swift", ".m", ".json", ".yml", ".yaml",
    ".txt", ".log", ".plist", ".xml", ".swift", ".pbxproj", "",
}
BINARY_SUFFIXES = {".png", ".ppm", ".ttf", ".ttc", ".a", ".o", ".so", ".a", ".dylib"}


def tracked_files():
    listed = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True
    ).stdout.split(b"\0")
    return [ROOT / name.decode() for name in listed if name]


def main() -> int:
    findings = []
    for path in tracked_files():
        if path.suffix.lower() in BINARY_SUFFIXES:
            continue
        try:
            text = path.read_text()
        except (UnicodeDecodeError, IsADirectoryError, FileNotFoundError):
            continue
        for line_number, line in enumerate(text.splitlines(), 1):
            if any(allow.search(line) for allow in ALLOW):
                continue
            for name, pattern in PATTERNS:
                if pattern.search(line):
                    findings.append((name, path.relative_to(ROOT), line_number, line.strip()[:160]))
    if findings:
        print("personal identifiers found in tracked files:", file=sys.stderr)
        for name, path, line_number, line in findings:
            print(f"  {name}: {path}:{line_number}: {line}", file=sys.stderr)
        return 1
    print("secret-scan: no personal identifiers in tracked files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
