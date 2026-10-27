#!/usr/bin/env python3
"""Run upstream Rime console against original tiny UI fixtures, never user settings.
以自有小型 UI 夹具运行上游 Rime 控制台，不读取或修改用户输入法配置。
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
lock = ROOT / 'third_party/ime/sources.lock.json'
fingerprint = hashlib.sha256(lock.read_bytes()).hexdigest()
console = ROOT / 'target/ime-deps' / fingerprint / 'rime-build/bin/rime_api_console'
out = ROOT / 'evidence/rime-smoke'
work = out / 'session'
out.mkdir(parents=True, exist_ok=True)
report = {'complete': False, 'rust_integration_complete': False, 'formal_proof_complete': False,
          'scope': 'Upstream native console only, tiny original dictionary, isolated directory',
          'lock_sha256': fingerprint, 'fixtures': {}}
def save():
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
save()
try:
    if work.exists():
        raise RuntimeError('session already exists; preserve evidence and choose a fresh run directory')
    work.mkdir()
    for source in (ROOT / 'experiments/ime-smoke').glob('*.yaml'):
        report['fixtures'][source.name] = hashlib.sha256(source.read_bytes()).hexdigest()
        shutil.copyfile(source, work / source.name)
    if not console.is_file():
        raise RuntimeError('Build pinned librime first with scripts/ime-deps.py --component rime')
    report['console_sha256'] = hashlib.sha256(console.read_bytes()).hexdigest()
    result = subprocess.run([str(console)], input='nihao\n{space}\nshijie{space}\nexit\n',
                            cwd=work, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60)
    (out / 'console.log').write_text(result.stdout)
    report['exit_code'] = result.returncode
    report['log_sha256'] = hashlib.sha256((out / 'console.log').read_bytes()).hexdigest()
    if result.returncode or 'commit: 你好' not in result.stdout or 'commit: 世界' not in result.stdout:
        raise RuntimeError('missing expected native commits; see console.log')
    report['native_smoke_complete'] = True
    report['complete'] = True
    save()
except Exception as error:
    report['failure'] = str(error)
    save()
    raise
