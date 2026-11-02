#!/usr/bin/env bash
# Basic closeout regression. Invoke through scripts/verify-in-vm.sh so compilation
# and tests stay inside the configured build VM.
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

run() {
    printf '\n$'
    printf ' %q' "$@"
    printf '\n'
    "$@"
}

run python3 scripts/check-blitz-native-dependencies.py
run python3 scripts/test-audit-closeout.py
run python3 scripts/audit-closeout.py --output "${CLOSEOUT_AUDIT_DIR:-evidence/final-closeout-audit}"
run cargo fmt --all -- --check
run cargo check --locked --all-targets
run cargo check --locked --features native-engine-cpu --all-targets
run cargo check --locked --features native-engine-cpu,gpu-pending --all-targets
run cargo test --locked

if [[ -n "${JASPER_UI_TEST_FONT:-}" ]]; then
    run cargo test --locked --features native-engine-cpu --lib
fi

if [[ "${CLOSEOUT_RUN_NATIVE_INTEGRATION:-0}" == "1" ]]; then
    if [[ -z "${JASPER_UI_TEST_FONT:-}" ]]; then
        echo 'CLOSEOUT_RUN_NATIVE_INTEGRATION=1 requires JASPER_UI_TEST_FONT' >&2
        exit 2
    fi
    run cargo test --locked --features native-engine-cpu \
        --test native_blitz_engine \
        --test native_cpu_acceptance \
        --test native_frame_differential \
        --test native_frame_transaction \
        --test native_update_contract \
        --test native_component_update_contract
fi

if [[ "${CLOSEOUT_RUN_GPU:-0}" == "1" ]]; then
    run cargo test --locked --features native-engine-cpu,gpu-pending \
        --test native_gpu_host_view \
        --test native_gpu_render \
        --test native_gpu_frame_damage \
        --test native_font_synthesis
fi

echo
echo 'closeout baseline passed'
