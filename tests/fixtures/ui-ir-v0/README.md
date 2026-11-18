# UI IR v0 Rust fixture copies

Canonical source:

- Repository: `jasper-language/jasper-appfw`
- Branch: `codex/ir-v0`
- Commit: `1e55d96`
- Directories: `docs/ui-ir-v0/fixtures/` and `docs/ui-ir-v0/schema/`

These copies are executed by the Rust consumer tests; `typed-properties-v0.json` pins the
Native consumer profile to the public Typed Property Registry. The appfw copies remain authoritative.
When a canonical file changes, update these copies and the expected FNV-1a hashes in
`tests/ui_ir_v0.rs` together.
