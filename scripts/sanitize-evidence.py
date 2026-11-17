#!/usr/bin/env python3
"""Replace machine-local home paths in tracked evidence and keep recorded hashes consistent.
将已跟踪证据中的本机主目录路径替换为占位符，并同步证据内记录的哈希。
"""
import hashlib
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HOME = "/" + "home/"
# Worktree checkouts (`-itemN`, `-verus`) are the same repository; an earlier pass may already have
# turned the home prefix into `<home>`, so that form is accepted too.
# 工作树（`-itemN`、`-verus`）同属本仓库；早先一轮可能已把主目录前缀换成 `<home>`，该形式同样接受。
REPO = re.compile(r"(?:" + re.escape(HOME) + r"[\w.-]+|<home>)(?:/[\w.-]+)*?/jasper-ui-engine(?:-item\d+|-verus)?(?=[/\s\"'`:,)\]]|$)", re.M)
OTHER = re.compile(re.escape(HOME) + r"[\w.-]+")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def clean(text):
    return OTHER.sub("<home>", REPO.sub("<repo>", text))


def main():
    listed = subprocess.run(["git", "ls-files", "-m", "-c", "evidence"], cwd=ROOT, check=True,
                            capture_output=True, text=True).stdout.split()
    files = {}
    for name in sorted(set(listed)):
        path = ROOT / name
        if not path.is_file():
            continue
        data = path.read_bytes()
        try:
            files[path] = data.decode()
        except UnicodeDecodeError:
            continue
    renamed = {}
    for path, text in files.items():
        updated = clean(text)
        if updated != text:
            renamed[digest(text.encode())] = digest(updated.encode())
            path.write_text(updated)
            files[path] = updated
    # A report that records a sanitized file's hash changes too, so repeat until stable.
    # 记录了被改文件哈希的报告本身也会变化，因此重复到稳定。
    while renamed:
        pending = {}
        for path, text in files.items():
            updated = text
            for old, new in renamed.items():
                updated = updated.replace(old, new)
            if updated != text:
                pending[digest(text.encode())] = digest(updated.encode())
                path.write_text(updated)
                files[path] = updated
        renamed = pending
    print("evidence files scanned:", len(files))


if __name__ == "__main__":
    main()
