# Long-term rendering and text dependencies

Parley and Vello CPU/GPU are long-term foundations, excluded from the strangler
plan. Scintilla/Lexilla and Keyman/Rime retain the explicit replacement plan.
Third-party trust boundaries and own-glue verification still apply.

Chosen stack: Vello CPU 0.3.0, Vello GPU 0.3.0, Parley 0.12.0.
Pinned archives, upstream license files and SHA-256 are in sources.lock.json.
Status: dependency manifest candidates only; no completed root build, adapter,
GPU device execution, text-edit loop or formal acceptance is claimed.
Cargo.lock has been regenerated inside the capped VM. The Parley adapter is
compile-checked there successfully; this is not a six-stage acceptance result.

CPU route disables GPU, PNG and worker-thread defaults. Parley disables system
font discovery; hosts must inject font/resource bytes. GPU is independently
optional and requires host device/backend configuration and validation; enabling
its dependency is not a portable GPU host implementation. Both render routes
must consume the same prepared Parley layout and UI-owned scene semantics.
No compositor, OS service or Canvas API is introduced by this selection.

The libraries and their transitive native/platform/code-generation dependencies
remain unproved trusted dependencies. Own Rust glue needs the ordered six-stage
gate. Their use on the system-built-in host is unverified. Record unsupported
features instead of expanding this integration into a third-party repair project.

Skia candidate source/tests are archived in experiments/deferred-skia-integration.
Historical evidence is retained. Canvas is shelved; optional tiny-skia remains
legacy rollback only, disabled by default. cosmic-text is also legacy opt-in.
Neither is the chosen long-term text/render route.
