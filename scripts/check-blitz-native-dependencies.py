#!/usr/bin/env python3
"""Check active native candidate dependency pin/features, not backend correctness."""
import hashlib
import json
from pathlib import Path
import tomllib

REVISION = '52739472460824ec7f1624ab135c09a4bbc99219'
REPOSITORY = 'https://github.com/DioxusLabs/blitz'
ARCHIVE_SHA256 = '2dc58362288e4f3837846da491c2f70c41048e2a61945bd12b91c4e97772ab9f'

def require(condition, message):
    if not condition:
        raise ValueError(message)

def check(root):
    manifest = tomllib.loads((root / 'experiments/blitz-host/Cargo.toml').read_text())
    dependencies = manifest['dependencies']
    require(set(dependencies) == {'jasper-ui-engine'}, 'diagnostic must consume only the root engine')
    engine = dependencies['jasper-ui-engine']
    require(engine['path'] == '../..' and engine['default-features'] is False
            and engine['features'] == ['native-blitz-pending'], 'diagnostic root native entry')
    require(manifest['features']['cpu-pending'] == ['jasper-ui-engine/native-blitz-cpu-pending'],
            'diagnostic CPU feature must forward to the root engine')
    sections = [manifest, *manifest.get('target', {}).values()]
    for section in sections:
        for kind in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            names = {value.get('package', name) if isinstance(value, dict) else name
                     for name, value in section.get(kind, {}).items()}
            require(not ({'blitz-html', 'blitz-shell', 'blitz-net'} & names),
                    'native target selected HTML/shell/network, including aliases')
    require(manifest['features']['default'] == [], 'candidate must remain opt-in')
    native_manifest = tomllib.loads((root / 'Cargo.toml').read_text())
    require(native_manifest['features']['default'] == [], 'root candidate must remain opt-in')
    for name in ('blitz-dom', 'blitz-traits', 'blitz-paint'):
        dependency = native_manifest['dependencies'][name]
        require(dependency['git'] == REPOSITORY and dependency['rev'] == REVISION
                and dependency['default-features'] is False and dependency['optional'] is True,
                'root native dependency: ' + name)
    native_lock = tomllib.loads((root / 'Cargo.lock').read_text())
    native_packages = [p for p in native_lock['package'] if p['name'].startswith('blitz-')]
    require(bool(native_packages), 'missing root resolved Blitz packages')
    for package in native_packages:
        require(package['source'] == f'git+{REPOSITORY}?rev={REVISION}#{REVISION}',
                'root resolved dependency: ' + package['name'])
    lock = tomllib.loads((root / 'experiments/blitz-host/Cargo.lock').read_text())
    packages = [p for p in lock['package'] if p['name'].startswith('blitz-')]
    require(bool(packages), 'missing resolved Blitz packages')
    for package in packages:
        require(package['source'] == f'git+{REPOSITORY}?rev={REVISION}#{REVISION}', package['name'])
    revision = json.loads((root / 'third_party/blitz/revision.json').read_text())
    require(revision['revision'] == REVISION, 'archive revision')
    require(revision['repository'] == REPOSITORY, 'archive repository')
    require(revision['sha256'] == ARCHIVE_SHA256, 'archive recorded digest')
    require(hashlib.sha256((root / 'third_party/blitz/source.tar.gz').read_bytes()).hexdigest() == ARCHIVE_SHA256, 'archive content digest')
    return {'status':'passed', 'revision':REVISION, 'scope':'root and diagnostic native manifests/locks, resolved Blitz git sources, archived source digest',
            'limits':['Not a license audit, implementation proof, or full transitive feature audit.']}

if __name__ == '__main__':
    print(json.dumps(check(Path(__file__).resolve().parents[1]), indent=2))
