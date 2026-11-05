#!/usr/bin/env python3
"""Check docs/render-tcb.md against Cargo.toml and Cargo.lock.

The register names the third-party rendering code the proofs stop at. It must not drift: every crate
the render features depend on directly is listed at its locked version, every listed crate is really
in the closure, and any change to the closure changes the recorded count and digest.
登记表列出证明止步处的第三方渲染代码，不得漂移：渲染 feature 的每个直接依赖都按锁定版本列出，所列 crate
确在闭包内，闭包的任何变化都会改变记录的数量与摘要。
"""
import argparse, hashlib, json, re, sys, tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RENDER_FEATURES = ["gpu-pending"]
# Below the direct dependencies, but the code the GPU path actually runs on.
# 不是直接依赖，但 GPU 路径实际运行其上。
REQUIRED_BELOW = {"wgpu-core", "wgpu-hal", "wgpu-types", "naga", "vello_cpu", "kurbo"}


def feature_deps(manifest, names):
    seen, deps, stack = set(), set(), list(names)
    while stack:
        name = stack.pop()
        if name in seen:
            continue
        seen.add(name)
        for item in manifest["features"][name]:
            if item.startswith("dep:"):
                deps.add(item[4:])
            elif "/" not in item:
                stack.append(item)
    return deps


def closure(lock, roots):
    packages = {}
    by_name = {}
    for package in lock["package"]:
        # Same name and version can come from two sources, and a lock entry names the source
        # without its commit fragment.
        # 同名同版本可能来自两个来源；锁文件条目写来源时不带提交片段。
        key = (package["name"], package["version"], package.get("source", "").split("#")[0])
        packages[key] = package
        by_name.setdefault(package["name"], []).append(key)
    def resolve(text):
        parts = text.split(" ")
        keys = by_name.get(parts[0], [])
        if len(parts) >= 2:
            keys = [key for key in keys if key[1] == parts[1]]
        if len(parts) >= 3:
            keys = [key for key in keys if key[2] == parts[2].strip("()")]
        if len(keys) != 1:
            raise SystemExit(f"ambiguous or missing lock entry: {text}")
        return keys[0]
    # The root package's own entries say which version a name means when several are locked.
    # 同名多版本时，以根包自身条目确定版本。
    own = {text.split(" ")[0]: text for text in packages[by_name["jasper-ui-engine"][0]]["dependencies"]}
    seen, stack = set(), [resolve(own[root]) for root in roots]
    while stack:
        key = stack.pop()
        if key in seen:
            continue
        seen.add(key)
        stack.extend(resolve(dep) for dep in packages[key].get("dependencies", []))
    return seen


def digest_of(keys):
    text = "".join(f"{name}@{version} {source}\n" for name, version, source in sorted(keys))
    return hashlib.sha256(text.encode()).hexdigest()


def check(manifest_text, lock_text, doc_text):
    """Return (errors, facts)."""
    manifest, lock = tomllib.loads(manifest_text), tomllib.loads(lock_text)
    direct = feature_deps(manifest, RENDER_FEATURES)
    keys = closure(lock, sorted(direct))
    locked = {}
    for name, version, _ in keys:
        locked.setdefault(name, set()).add(version)
    rows = {}
    errors = []
    for name, version in re.findall(r"^\| `([^`]+)` \| `([^`]+)` \|", doc_text, re.M):
        if name in rows:
            errors.append(f"listed twice: {name}")
        rows[name] = version
    for name in sorted(direct | REQUIRED_BELOW):
        if name not in rows:
            errors.append(f"not registered: {name}")
    for name, version in sorted(rows.items()):
        if version not in locked.get(name, set()):
            errors.append(f"stale row: {name} {version}; closure has {sorted(locked.get(name, []))}")
    facts = {"direct": sorted(direct), "registered": rows, "closure_count": len(keys), "closure_sha256": digest_of(keys)}
    count = re.search(r"闭包数量：(\d+)", doc_text)
    digest = re.search(r"闭包摘要：`([0-9a-f]{64})`", doc_text)
    if not count or int(count.group(1)) != len(keys):
        errors.append(f"closure count differs: {len(keys)}")
    if not digest or digest.group(1) != facts["closure_sha256"]:
        errors.append(f"closure digest differs: {facts['closure_sha256']}")
    return errors, facts


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write-evidence", action="store_true")
    args = parser.parse_args()
    sources = {name: (ROOT / name).read_text() for name in ["Cargo.toml", "Cargo.lock", "docs/render-tcb.md"]}
    errors, facts = check(sources["Cargo.toml"], sources["Cargo.lock"], sources["docs/render-tcb.md"])
    for error in errors:
        print(error)
    if errors:
        return 1
    if args.write_evidence:
        out = ROOT / "evidence/render-tcb"
        out.mkdir(parents=True, exist_ok=True)
        report = {"check": "scripts/check-render-tcb.py", "status": "register_matches_lock",
                  "sources": {name: hashlib.sha256(text.encode()).hexdigest() for name, text in sources.items()},
                  "limits": "Cargo.lock closure over all platforms and optional paths, so wider than any one build; a listed crate is trusted, not verified",
                  **facts}
        (out / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(f"render TCB register matches: {len(facts['registered'])} rows, closure {facts['closure_count']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
