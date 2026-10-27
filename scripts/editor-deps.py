#!/usr/bin/env python3
"""Prepare/build pinned upstream editor dependencies; not a Rust delivery gate.
准备/构建固定版本的上游编辑依赖；不替代 Rust 交付验证。
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
DEPS = ROOT / 'third_party/editor'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare-only', action='store_true')
    parser.add_argument('--jobs', type=int, default=2)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        parser.error('--jobs must be between 1 and 8')
    lock = DEPS / 'sources.lock.json'
    data = json.loads(lock.read_text())
    fingerprint = sha(lock)
    work = ROOT / 'target/editor-deps' / fingerprint
    out = ROOT / 'evidence/editor-deps'
    out.mkdir(parents=True, exist_ok=True)
    report = {'started_at': datetime.now(timezone.utc).isoformat(),
              'lock_sha256': fingerprint, 'script_sha256': sha(Path(__file__)),
              'upstream_checks_complete': False, 'rust_integration_complete': False,
              'jasper_os_host_complete': False, 'formal_proof_complete': False,
              'stages': [], 'dependencies': data['dependencies']}

    def save():
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')

    def verify_sources():
        # Match actual compiler inputs against every original source archive member.
        # 将实际编译输入逐文件对照原始归档，避免可变缓存冒充固定源。
        count = 0
        for dep in data['dependencies']:
            with tarfile.open(DEPS / dep['archive'], 'r:gz') as tar:
                for member in tar.getmembers():
                    if member.isfile():
                        source = work / member.name
                        expected = tar.extractfile(member).read()
                        if not source.is_file() or source.read_bytes() != expected:
                            raise RuntimeError('upstream source changed: ' + member.name)
                        count += 1
        return count

    save()
    try:
        for dep in data['dependencies']:
            archive = DEPS / dep['archive']
            if sha(archive) != dep['sha256']:
                raise RuntimeError('source checksum mismatch: ' + dep['name'])
            directory = work / dep['source_root']
            stamp = work / (dep['name'] + '.extracted')
            if stamp.exists():
                if stamp.read_text() != dep['sha256'] or not directory.is_dir():
                    raise RuntimeError('inconsistent extraction cache')
                continue
            # Reject traversal, links and special members before extracting any data.
            # 提取前拒绝越界路径、链接与特殊文件，不覆盖未登记的缓存目录。
            if directory.exists():
                raise RuntimeError('incomplete extraction cache: ' + str(directory))
            work.mkdir(parents=True, exist_ok=True)
            with tarfile.open(archive, 'r:gz') as tar:
                members = tar.getmembers()
                for member in members:
                    relative = Path(member.name)
                    if (relative.is_absolute() or '..' in relative.parts
                            or not relative.parts or relative.parts[0] != dep['source_root']
                            or not (member.isfile() or member.isdir())):
                        raise RuntimeError('unsafe archive member: ' + member.name)
                tar.extractall(work, members=members, filter='data')
            if (directory / 'License.txt').read_bytes() != (DEPS / dep['license_file']).read_bytes():
                raise RuntimeError('license copy mismatch')
            stamp.write_text(dep['sha256'])
        report['source_files_verified'] = verify_sources()
        report['sources_verified'] = True
        save()
        if args.prepare_only:
            print('Pinned editor sources verified; build and Rust integration pending.')
            return
        for tool in ['g++', 'make']:
            report[tool] = subprocess.check_output([tool, '--version'], text=True).splitlines()[0]
        commands = [
            ('scintilla-core-tests', 'scintilla/test/unit', ['make', 'test', 'OPTIMIZATION=-O1']),
            ('lexilla-library', 'lexilla/src', ['make', 'CXXFLAGS=-O1']),
            ('lexilla-unit-tests', 'lexilla/test/unit', ['make', 'test', 'OPTIMIZATION=-O1']),
            ('lexilla-lexer-tests', 'lexilla/test', ['make', 'test', 'CXXFLAGS=-O1']),
        ]
        for name, directory, command in commands:
            command += ['-j' + str(args.jobs)]
            stage = {'name': name, 'command': command, 'status': 'running'}
            report['stages'].append(stage)
            save()
            log = out / (name + '.log')
            with log.open('w') as stream:
                result = subprocess.run(command, cwd=work / directory,
                                        stdout=stream, stderr=subprocess.STDOUT, timeout=1800)
            stage.update(exit_code=result.returncode, log_sha256=sha(log))
            if result.returncode:
                stage['status'] = 'failed'
                raise RuntimeError(name + ' failed; see ' + str(log))
            stage['status'] = 'passed_upstream_tests' if 'tests' in name else 'built'
            save()
            print(name + ': ' + stage['status'], flush=True)
        report['artifacts'] = {str(p.relative_to(work)): sha(p)
                               for p in (work / 'lexilla/bin').iterdir()
                               if p.is_file() and p.suffix in {'.a', '.so', '.dylib', '.dll'}}
        report['source_files_verified_after_build'] = verify_sources()
        report['upstream_checks_complete'] = True
        report['finished_at'] = datetime.now(timezone.utc).isoformat()
        save()
    except Exception as error:
        report['failure'] = str(error)
        save()
        raise


if __name__ == '__main__':
    main()
