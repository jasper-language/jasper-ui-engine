# Jasper UI closeout audit

This report is an audit snapshot, not a delivery acceptance record.

- branch: `codex/item6-closeout`
- commit: `22372ff1daad8e6cfc0fd2a0e1b99c560c651d24`
- explicit item 1-5 completions: 0/5
- final report materially covers HEAD: `false`
- missing required evidence: 0
- missing final-report references: 17
- stale/missing recorded source hashes: 8
- branch refs included: `included`
- tool configuration: `passed`
- evidence reports indexed: 24

## Checklist readings

| Item | Reading |
|---:|---|
| 1 | `not_complete` |
| 2 | `not_complete` |
| 3 | `not_complete` |
| 4 | `not_complete` |
| 5 | `not_complete` |

## Missing final-audit evidence

- `evidence/blitz-framework-update-contract-pending/report.json`
- `evidence/blitz-root-hello-migration/report.json`
- `evidence/blitz-root-pointer-migration/report.json`
- `evidence/blitz-root-window-migration/report.json`
- `evidence/input-lifecycle-chain/report.json`
- `evidence/native-backend-frame-chain/report.json`
- `evidence/native-backend-submission-chain/report.json`
- `evidence/native-dpi-scale-admission-chain/report.json`
- `evidence/native-frame-orchestration-chain/report.json`
- `evidence/native-headless-forwarding-chain/report.json`
- `evidence/native-host-surface-orchestration-chain/report.json`
- `evidence/native-map-ordered-replay/report.json`
- `evidence/native-subtree-ordered-replay/report.json`
- `evidence/native-subtree-partition-candidate/report.json`
- `evidence/ui-composition-chain/report.json`
- `evidence/ui-input-binding-chain/report.json`
- `evidence/validation-chain/report.json`

## Stale or missing source correspondence

- `src/native_adapter/facade.rs` in `evidence/scene-update-error-contract-chain/report.json`: `stale`
- `src/native_adapter/facade.rs` in `evidence/native-gpu-host-target-pending/report.json`: `stale`
- `src/native_adapter/native_scene.rs` in `evidence/native-gpu-host-target-pending/report.json`: `stale`
- `scripts/verify-version.py` in `evidence/native-font-bold-alias-chain/report.json`: `stale`
- `src/native_adapter/native_scene.rs` in `evidence/native-font-bold-alias-chain/report.json`: `stale`
- `src/native_adapter/native_view.rs` in `evidence/native-font-bold-alias-chain/report.json`: `stale`
- `Cargo.toml` in `evidence/full-scene-migration-pending/report.json`: `stale`

Run `python3 scripts/audit-closeout.py --require-ready` only after items 2-5
have been integrated and their evidence has been regenerated.
