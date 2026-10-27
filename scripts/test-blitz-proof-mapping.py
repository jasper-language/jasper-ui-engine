#!/usr/bin/env python3
"""Exercise proof-evidence rejection against disposable copies, without a solver run."""
import copy
import json
from pathlib import Path
import runpy
import shutil
import tempfile

ROOT = Path(__file__).resolve().parents[1]
AUDIT = runpy.run_path(str(ROOT / 'scripts/check-blitz-proof-mapping.py'))
REGISTRY = runpy.run_path(str(ROOT / 'scripts/verify-version.py'))


def require(condition, detail):
    if not condition:
        raise RuntimeError(detail)


def main():
    baseline = AUDIT['check'](ROOT)
    require(baseline['current'], baseline)
    with tempfile.TemporaryDirectory(prefix='jasper-ui-proof-correspondence-') as temporary:
        root = Path(temporary)
        files = {'scripts/verify-version.py',
                 'verification/blitz-ancestry/hidden_ancestry.rs',
                 'evidence/blitz-hidden-ancestry-proof-pending/report.json'}
        files.update('evidence/blitz-hidden-ancestry-proof-pending/' + name
                     for name in ('verus.log', 'tools.log', 'ignore-ancestor.log', 'no-decrement.log'))
        files.update('verification/blitz-ancestry/' + name
                     for name in ('hidden_ignore_ancestor.rs', 'hidden_no_decrement.rs'))
        for key in AUDIT['KEYS']:
            directory = Path('evidence') / REGISTRY['KERNELS'][key]['evidence']
            files.update(str(directory / name) for name in ('report.json', 'kernel.rs', 'verus.log'))
            for label, _, _ in REGISTRY['KERNELS'][key]['mutations']:
                files.add(str(directory / ('negative_' + label.replace('-', '_') + '.rs')))
                files.add(str(directory / ('negative-' + label + '.log')))
            for dependency in REGISTRY['dependency_order'](key):
                files.add(REGISTRY['KERNELS'][dependency]['source'])
        for file in files:
            target = root / file
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / file, target)
        require(AUDIT['check'](root)['current'], 'copied baseline mismatch')
        results = []

        def reject(label, file, mutate, affected):
            target = root / file
            original = target.read_bytes()
            try:
                mutate(target)
                result = AUDIT['check'](root)
                entries = {entry['kernel']: entry for entry in result['entries']}
                require(not result['current'], label)
                require(all(not entries[key]['current'] for key in affected), (label, result))
                results.append({'negative': label, 'rejected': True, 'affected': affected})
            finally:
                target.write_bytes(original)
            require(AUDIT['check'](root)['current'], 'copied baseline mismatch'), label + ': restore failed'

        def change_body(target, old, new):
            source = target.read_text()
            require(source.count(old) == 1, old)
            target.write_text(source.replace(old, new))

        reject('changed-public-version-body', 'src/native_adapter/facade.rs',
               lambda p: change_body(p, 'self.inner.component_version()', 'Some(0)'),
               ['text-display-version'])
        reject('changed-scene-close-dependency', 'src/native_adapter/native_scene.rs',
               lambda p: change_body(p, 'caller==self.owner && self.document.close()',
                                      'caller!=self.owner && self.document.close()'),
               ['scene-host-close'])
        reject('changed-hidden-ancestry-body', 'src/native_adapter/native_scene.rs',
               lambda p: change_body(p, 'hidden |= semantic.hidden;', 'hidden = semantic.hidden;'),
               ['source-hidden-ancestry'])
        reject('changed-scene-snapshot-identity', 'src/native_adapter/native_scene.rs',
               lambda p: change_body(p, 'Some((self.scene_version,self.node_count))', 'Some((self.scene_version,0))'),
               ['scene-snapshot-identity'])
        reject('missing-scene-snapshot-proof', 'evidence/scene-snapshot-identity-kernel/report.json',
               lambda p: p.unlink(), ['scene-snapshot-identity'])
        reject('changed-semantic-tag-admission', 'src/native_adapter/native_scene.rs',
               lambda p: change_body(p, 'role_uses_native_button(current)==role_uses_native_button(next)', 'role_uses_native_button(current)!=role_uses_native_button(next)'),
               ['semantic-role-tag'])
        reject('changed-public-scene-identity-owner', 'src/native_adapter/facade.rs',
               lambda p: change_body(p, 'self.inner.snapshot_identity(owner)', 'None'),
               ['scene-host-identity'])
        public_report = 'evidence/text-display-close-kernel/report.json'
        reject('missing-public-close-proof', public_report, lambda p: p.unlink(), ['text-display-close'])

        def invalidate_report(target):
            data = json.loads(target.read_text())
            data['status'] = 'timeout'
            target.write_text(json.dumps(data))
        reject('nonproved-public-close-report', public_report, invalidate_report, ['text-display-close'])

        def invalidate_controls(target, mode):
            data = json.loads(target.read_text())
            if mode == 'flag':
                data['negative_control_rejected'] = False
            elif mode == 'missing':
                data['negative_controls'].pop()
            else:
                data['negative_controls'][0]['exit_code'] = 124
            target.write_text(json.dumps(data))
        for mode in ('flag', 'missing', 'timeout'):
            reject('invalid-negative-control-' + mode, public_report,
                   lambda p, mode=mode: invalidate_controls(p, mode), ['text-display-close'])
        for label, file in (
            ('missing-saved-proof', 'kernel.rs'),
            ('missing-positive-log', 'verus.log'),
            ('missing-negative-log', 'negative-skip-retirement.log'),
            ('missing-negative-source', 'negative_skip_retirement.rs'),
        ):
            reject(label, 'evidence/text-display-close-kernel/' + file,
                   lambda p: p.unlink(), ['text-display-close'])
        reject('corrupt-saved-proof', 'evidence/text-display-close-kernel/kernel.rs',
               lambda p: p.write_text('corrupt proof'), ['text-display-close'])
        reject('missing-hidden-ancestry-log', 'evidence/blitz-hidden-ancestry-proof-pending/verus.log',
               lambda p: p.unlink(), ['source-hidden-ancestry'])
        reject('missing-hidden-ancestry-negative', 'verification/blitz-ancestry/hidden_no_decrement.rs',
               lambda p: p.unlink(), ['source-hidden-ancestry'])
        reject('changed-native-backend-frame-replacement', 'src/backend.rs',
               lambda p: change_body(p, 'self.canvas=canvas;Ok(())', 'let _=canvas;Ok(())'),
               ['native-backend-frame-import'])
        result = copy.deepcopy(baseline)
        result['negative_controls'] = results
        result['test_scope'] = 'Source/evidence correspondence only; no Rust acceptance gate or solver run.'
        print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
