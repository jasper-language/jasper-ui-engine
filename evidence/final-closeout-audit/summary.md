# Jasper UI closeout audit

This report is an audit snapshot, not a delivery acceptance record.

- branch: `dev/ui38-batch-unique`
- commit: `8b83c399c0922d92832ee114eb9fff067f22708c`
- explicit item 1-5 completions: 5/5
- final report materially covers HEAD: `false`
- missing required evidence: 0
- missing final-report references: 0
- stale/missing recorded source hashes: 10
- branch refs included: `included`
- tool configuration: `passed`
- evidence reports indexed: 423

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

- `scripts/audit-closeout.py` in `evidence/final-checklist-audit/report.json`: `stale`
- `src/scene.rs` in `evidence/final-checklist-audit/report.json`: `stale`

Run `python3 scripts/audit-closeout.py --require-ready` only after items 2-5
have been integrated and their evidence has been regenerated.
