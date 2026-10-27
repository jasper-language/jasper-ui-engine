#!/usr/bin/env python3
"""Fail closed when a reused native-adapter proof no longer maps to current bodies/specs.
This checks evidence correspondence only. It does not execute a prover or accept the adapter.
"""
import argparse
import hashlib
import json
from pathlib import Path
import runpy

KEYS = (
    'ui-composition-identity', 'composition-current-token', 'ui-view-owner',
    'ui-snapshot-admission', 'ui-input-payload', 'ui-composition-range',
    'ui-view-slot', 'ui-view-read-borrow', 'ui-view-write-borrow', 'ui-update-next',
    'native-component-version', 'native-activation-admission', 'component-update-support',
    'scene-work-capacity', 'scene-update-admission', 'source-insert-index',
    'text-display-version', 'text-display-close', 'scene-host-close', 'cpu-frame-bytes', 'capture-cancel-policy', 'native-host-context', 'scene-snapshot-identity', 'scene-host-identity', 'semantic-role-tag', 'native-backend-frame-import',
)


def digest(text):
    return hashlib.sha256(text.encode()).hexdigest()


def check(root):
    registry = runpy.run_path(str(root / 'scripts/verify-version.py'))
    entries = []
    for key in KEYS:
        try:
            config = registry['KERNELS'][key]
            report_path = root / 'evidence' / config['evidence'] / 'report.json'
            report = json.loads(report_path.read_text())
            ordered = registry['dependency_order'](key)
            extracted = {name: registry['extract'](root, name) for name in ordered}
            specs = '\n'.join(registry['KERNELS'][name].get('spec_prelude', '') for name in ordered)
            proof = 'use vstd::prelude::*;\nverus! {\n' + specs + '\n' + '\n'.join(
                extracted[name][1] for name in ordered) + '\n}\n'
            evidence = report_path.parent
            positive = registry['summary']((evidence / 'verus.log').read_text())
            controls_match = True
            for label, pattern, replacement in config['mutations']:
                negative_source = evidence / ('negative_' + label.replace('-', '_') + '.rs')
                negative_log = (evidence / ('negative-' + label + '.log')).read_text()
                stats = registry['summary'](negative_log)
                controls_match &= (proof.count(pattern) == 1
                    and negative_source.read_text() == proof.replace(pattern, replacement, 1)
                    and stats is not None and stats[1] > 0
                    and 'postcondition not satisfied' in negative_log)
            matches = {
                'proved_report': report['status'] == 'proved' and report['exit_code'] == 0,
                'property': report['property'] == config['property'],
                'negative_controls': report.get('negative_control_rejected') is True
                    and len(report.get('negative_controls', [])) == len(config['mutations'])
                    and {item['name'] for item in report.get('negative_controls', [])}
                        == {item[0] for item in config['mutations']}
                    and all(item.get('rejected') is True and item.get('exit_code') == 1
                            for item in report.get('negative_controls', [])),
                'body': digest(extracted[key][0]) == report['kernel_sha256'],
                'dependencies': {name: digest(extracted[name][0]) for name in ordered if name != key}
                    == report['dependencies'],
                'spec_and_body': digest(proof) == report['proof_sha256'],
                'saved_proof': digest((evidence / 'kernel.rs').read_text()) == report['proof_sha256'],
                'positive_log': positive == (report['verified_obligations'], 0),
                'negative_artifacts': controls_match,
            }
            entries.append({'kernel': key, 'matches': matches, 'current': all(matches.values())})
        except (OSError, ValueError, KeyError) as error:
            entries.append({'kernel': key, 'current': False, 'error': str(error)})
    # Standalone map proof: preserve its exact production marker and annotated proof digest.
    # Current NativeScene/semantics declarations are checked by scene-host-close above.
    key = 'source-hidden-ancestry'
    try:
        source = (root / 'src/native_adapter/native_scene.rs').read_text()
        begin = '// BEGIN VERIFIED SOURCE HIDDEN ANCESTRY KERNEL\n'
        end = '// END VERIFIED SOURCE HIDDEN ANCESTRY KERNEL'
        if source.count(begin) != 1 or source.count(end) != 1:
            raise ValueError('hidden ancestry marker must be unique')
        body = source.split(begin)[1].split(end)[0]
        report = json.loads((root / 'evidence/blitz-hidden-ancestry-proof-pending/report.json').read_text())
        correspondence = report['correspondence']
        proof = (root / 'verification/blitz-ancestry/hidden_ancestry.rs').read_text()
        evidence = root / 'evidence/blitz-hidden-ancestry-proof-pending'
        positive = registry['summary']((evidence / 'verus.log').read_text())
        negatives_match = True
        for source_name, log_name, diagnostic in [
                ('hidden_ignore_ancestor.rs', 'ignore-ancestor.log', 'invariant not satisfied at end of loop body'),
                ('hidden_no_decrement.rs', 'no-decrement.log', 'decreases not satisfied at end of loop')]:
            source_path = 'verification/blitz-ancestry/' + source_name
            negative_log = (evidence / log_name).read_text()
            stats = registry['summary'](negative_log)
            negatives_match &= (digest((root / source_path).read_text()) == report['sources'][source_path]
                and stats is not None and stats[1] > 0 and diagnostic in negative_log)
        matches = {
            'saved_logs': positive == (report['Verus+Z3']['verified_including_reused_dependencies'], 0)
                and bool((evidence / 'tools.log').read_text().strip()),
            'negative_artifacts': negatives_match and report['Verus+Z3']['negative_controls_rejected'] == 2,
            'proved_report': report['status'] == 'stage1_complete_remaining_acceptance_pending'
                and report['Verus+Z3']['exit_code'] == 0,
            'body': digest(body) == correspondence['actual_body_sha256'],
            'spec_and_body': digest(proof) == correspondence['proof_sha256'],
            'recorded_ghost_erasure': correspondence['ghost_erasure_matches_production'] is True,
        }
        entries.append({'kernel': key, 'matches': matches, 'current': all(matches.values())})
    except (OSError, ValueError, KeyError) as error:
        entries.append({'kernel': key, 'current': False, 'error': str(error)})
    return {'kind': 'evidence_correspondence_only', 'whole_native_adapter_accepted': False,
            'current': all(entry['current'] for entry in entries), 'entries': entries,
            'limitations': ['No solver run or six-stage rerun is performed.',
                            'Whole-file hashes may differ outside extracted bodies; only listed bodies/dependencies/specs match.',
                            'Native composition, backend effects and third-party correctness remain outside this audit.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    result = check(args.root.resolve())
    print(json.dumps(result, indent=2))
    return 0 if result['current'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
