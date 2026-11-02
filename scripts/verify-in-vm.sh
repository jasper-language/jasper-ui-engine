#!/usr/bin/env bash
# UI verification only: no compiler or solver is launched on the desktop host.
# UI 验证仅在构建虚拟机执行，并为本次进程树设置资源上限。
set -euo pipefail
if (( $# == 0 )); then
    echo 'Usage: scripts/verify-in-vm.sh <command> [args...]' >&2
    exit 2
fi
ui_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# The caller supplies the VM entry point; this public repository does not embed
# host paths or private build-infrastructure names.
vm_run=${JASPER_UI_VM_RUN:-}
if [[ -z "$vm_run" ]]; then
    echo 'Set JASPER_UI_VM_RUN to the build VM entry script.' >&2
    exit 2
fi
if [[ ! -f "$vm_run" ]]; then
    echo "Build VM entry script not found: $vm_run" >&2
    exit 2
fi
# Lightweight source/configuration admission only; all compilation stays inside VM.
python3 "$ui_root/scripts/check-blitz-native-dependencies.py" >/dev/null
exec 9>/tmp/jasper-ui-rust-verification.lock
flock -n 9 || { echo 'Another UI verification run is active.' >&2; exit 75; }
# The original constrained profile remains the default. The current validation
# owner can explicitly raise memory, swap and wall time for one VM workload.
ui_work_seconds=${JASPER_UI_VM_TIMEOUT_SECONDS:-180}
case "$ui_work_seconds" in
    ''|*[!0-9]*) echo 'UI VM timeout must be an integer from 1 to 3600.' >&2; exit 2 ;;
esac
if (( ${#ui_work_seconds} > 4 )); then
    echo 'UI VM timeout must be an integer from 1 to 3600.' >&2
    exit 2
fi
ui_work_seconds=$((10#$ui_work_seconds))
if (( ui_work_seconds < 1 || ui_work_seconds > 3600 )); then
    echo 'UI VM timeout must be an integer from 1 to 3600.' >&2
    exit 2
fi
ui_memory=${JASPER_UI_VM_MEMORY:-3G}
if [[ ! "$ui_memory" =~ ^[3-8](\.5)?G$ ]]; then
    echo 'UI VM memory must be 3G through 8G in 0.5G steps.' >&2
    exit 2
fi
ui_swap=${JASPER_UI_VM_SWAP:-0G}
if [[ ! "$ui_swap" =~ ^([0-9]|1[0-9]|2[0-4])(\.5)?G$ ]]; then
    echo 'UI VM swap must be 0G through 24G in 0.5G steps.' >&2
    exit 2
fi
ui_transport_seconds=$((ui_work_seconds + 120))
timeout --kill-after=10s "${ui_transport_seconds}s" env VM_NICE=15 VM_KEEP_AWAKE=0 bash "$vm_run" \
    "$ui_root" systemd-run --user --scope --quiet --expand-environment=no \
    --property=MemoryMax="$ui_memory" --property=MemorySwapMax="$ui_swap" --property=TasksMax=128 \
    env CARGO_BUILD_JOBS=1 JASPER_UI_VERIFICATION_VM=1 \
    timeout --kill-after=10s "${ui_work_seconds}s" "$@"
