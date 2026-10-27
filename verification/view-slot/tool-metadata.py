"""Read verification tool versions inside the capped VM; no tests run here."""
import json
from pathlib import Path
import subprocess
root = Path(__file__).resolve().parents[2]
commands = {
    "rustc": ["rustc", "--version"],
    "miri": ["cargo", "+nightly-2026-10-17", "miri", "--version"],
    "cargo-fuzz": ["cargo", "+nightly-2026-10-17", "fuzz", "--version"],
    "kani": ["cargo", "kani", "--version"],
}
result = {}
for name, command in commands.items():
    output = subprocess.run(command, text=True, capture_output=True, timeout=30)
    result[name] = {"command": command, "exit_code": output.returncode,
                    "version_output": output.stdout + output.stderr}
path = root / "evidence/ui-view-slot-chain/tools.json"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(result, indent=2) + "\n")
