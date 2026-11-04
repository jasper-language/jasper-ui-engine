# Jasper UI closeout audit

This report is an audit snapshot, not a delivery acceptance record.

- branch: `dev/ui-jaso-plugin`
- commit: `09dc4438d6b680ffed22b9584143283d0ae87888`
- explicit item 1-5 completions: 5/5
- final report materially covers HEAD: `false`
- missing required evidence: 0
- missing final-report references: 0
- stale/missing recorded source hashes: 20
- branch refs included: `included`
- tool configuration: `passed`
- evidence reports indexed: 419

## Checklist readings

| Item | Reading |
|---:|---|
| 1 | `explicit_complete_current_scope` |
| 2 | `explicit_complete_current_scope` |
| 3 | `explicit_complete_current_scope` |
| 4 | `explicit_complete_current_scope` |
| 5 | `explicit_complete_current_scope` |

## Missing final-audit evidence

None.

## Stale or missing source correspondence

- `scripts/closeout-baseline.sh` in `evidence/final-checklist-audit/report.json`: `stale`
- `src/native_adapter/facade.rs` in `evidence/final-checklist-audit/report.json`: `stale`
- `src/native_adapter/native_scene.rs` in `evidence/final-checklist-audit/report.json`: `stale`
- `src/surface_lifecycle.rs` in `evidence/final-checklist-audit/report.json`: `stale`

Run `python3 scripts/audit-closeout.py --require-ready` only after items 2-5
have been integrated and their evidence has been regenerated.
