#!/usr/bin/env python3
"""Prepare/build pinned upstream input-method dependencies; not a Rust delivery gate.
准备/构建固定版本的上游输入法依赖；不替代 Rust 交付验证。
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
DEPS = ROOT / 'third_party/ime'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare-only', action='store_true')
    parser.add_argument('--jobs', type=int, default=2)
    parser.add_argument('--component', choices=['keyman', 'rime'], default='keyman')
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        parser.error('--jobs must be between 1 and 8')
    lock = DEPS / 'sources.lock.json'
    data = json.loads(lock.read_text())
    fingerprint = sha(lock)
    work = ROOT / 'target/ime-deps' / fingerprint
    out = ROOT / 'evidence/ime-deps' / args.component
    out.mkdir(parents=True, exist_ok=True)
    report = {'started_at': datetime.now(timezone.utc).isoformat(),
              'lock_sha256': fingerprint, 'script_sha256': sha(Path(__file__)),
              'upstream_checks_complete': False, 'framework_build_complete': False,
              'rust_integration_complete': False,
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
                    if member.issym():
                        link = work / member.name
                        if not link.is_symlink() or link.readlink() != Path(member.linkname):
                            raise RuntimeError('upstream symlink changed: ' + member.name)
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
                            or not (member.isfile() or member.isdir() or member.issym())):
                        raise RuntimeError('unsafe archive member: ' + member.name)
                    if member.issym():
                        target = (work / member.name).parent / member.linkname
                        if not target.resolve().is_relative_to((work / dep['source_root']).resolve()):
                            raise RuntimeError('escaping source symlink: ' + member.name)
                tar.extractall(work, members=members, filter='data')
            if (directory / dep['license_source']).read_bytes() != (DEPS / dep['license_file']).read_bytes():
                raise RuntimeError('license copy mismatch')
            stamp.write_text(dep['sha256'])
        report['source_files_verified'] = verify_sources()
        report['sources_verified'] = True
        save()
        if args.prepare_only:
            print('Pinned input-method sources verified; build and Rust integration pending.')
            return
        if args.component == 'keyman':
            report['component'] = 'keyman-core'
            source = work / 'keyman-18.0.255'
            build = work / 'keyman-build'
            generated = source / 'core/resources/meson.build'
            generated.parent.mkdir(parents=True, exist_ok=True)
            # Same resource copy as upstream core/build.sh; no patched runtime source.
            # 执行上游构建脚本同样的资源复制，不修改运行源码。
            generated.write_bytes((source / 'resources/build/meson/standard.meson.build').read_bytes())
            report['generated_build_resource_sha256'] = sha(generated)
            commands = [
                ['meson', 'setup', str(build), str(source / 'core'), '--wrap-mode=nofallback',
                 '-Dkeyman_core_tests=false', '--buildtype=debugoptimized', '-Doptimization=1'],
                ['meson', 'compile', '-C', str(build), '-j', str(args.jobs)],
            ]
            for index, command in enumerate(commands):
                log = out / ('keyman-build-' + str(index) + '.log')
                with log.open('w') as stream:
                    result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, timeout=900)
                stage = {'command': command, 'exit_code': result.returncode, 'log_sha256': sha(log)}
                report['stages'].append(stage)
                save()
                if result.returncode:
                    raise RuntimeError('Keyman Core build failed; see ' + str(log))
            report['source_files_verified_after_build'] = verify_sources()
            report['keyman_core_library_built'] = True
            report['upstream_tests_run'] = False
            report['pending'] = ['Keyman upstream tests, host/engine integration, transitive license inventory']
            report['artifacts'] = {str(p.relative_to(work)): sha(p) for p in build.rglob('libkeymancore*')
                                   if p.is_file() and not p.is_symlink()}
            save()
            print('Keyman Core library built; upstream tests and framework integration pending.', flush=True)
            return
        for tool in ['g++', 'cmake', 'ctest']:
            report[tool] = subprocess.check_output([tool, '--version'], text=True).splitlines()[0]
        prefix = work / 'install'
        rime = work / 'librime-1.17.0'
        rb = work / 'rime-build'
        common = ['-DCMAKE_BUILD_TYPE=RelWithDebInfo', '-DCMAKE_CXX_FLAGS_RELWITHDEBINFO=-O1 -g0 -DNDEBUG',
                  '-DCMAKE_INSTALL_PREFIX=' + str(prefix), '-DCMAKE_INSTALL_LIBDIR=lib']
        commands = [
            ('rime-configure', '.', ['cmake', '-S', str(rime), '-B', str(rb)] + common + [
                '-DENABLE_LOGGING=OFF', '-DENABLE_EXTERNAL_PLUGINS=OFF', '-DBUILD_TEST=ON',
                '-DBUILD_DATA=OFF', '-DENABLE_TIMESTAMP=OFF']),
            ('rime-build', '.', ['cmake', '--build', str(rb), '--parallel', str(args.jobs)]),
            ('rime-tests', '.', ['ctest', '--test-dir', str(rb), '--output-on-failure', '--timeout', '60']),
            ('rime-install-local', '.', ['cmake', '--install', str(rb)]),
        ]
        import os
        environment = os.environ.copy()
        environment['PKG_CONFIG_PATH'] = str(prefix / 'lib/pkgconfig') + ':' + environment.get('PKG_CONFIG_PATH', '')
        for name, directory, command in commands:
            stage = {'name': name, 'command': command, 'status': 'running'}
            report['stages'].append(stage)
            save()
            log = out / (name + '.log')
            with log.open('w') as stream:
                result = subprocess.run(command, cwd=work / directory,
                                        stdout=stream, stderr=subprocess.STDOUT, timeout=1800, env=environment)
            stage.update(exit_code=result.returncode, log_sha256=sha(log))
            if result.returncode:
                stage['status'] = 'failed'
                raise RuntimeError(name + ' failed; see ' + str(log))
            stage['status'] = 'passed_upstream_tests' if 'tests' in name else 'completed'
            if 'tests' in name and not __import__('re').search(r'100% tests passed, 0 tests failed out of ([1-9][0-9]*)', log.read_text()):
                raise RuntimeError('missing nonempty upstream test evidence: ' + name)
            save()
            print(name + ': ' + stage['status'], flush=True)
        report['artifacts'] = {str(p.relative_to(work)): sha(p)
                               for directory in [prefix / 'lib']
                               for p in directory.rglob('*')
                               if p.is_file() and not p.is_symlink() and ('.so' in p.name or p.suffix == '.a')}
        report['source_files_verified_after_build'] = verify_sources()
        report['rime_upstream_checks_complete'] = True
        report['upstream_checks_complete'] = False
        report['pending'] = ['Keyman Core build and host integration; transitive dependency license review']
        report['finished_at'] = datetime.now(timezone.utc).isoformat()
        save()
    except Exception as error:
        report['failure'] = str(error)
        save()
        raise


if __name__ == '__main__':
    main()
