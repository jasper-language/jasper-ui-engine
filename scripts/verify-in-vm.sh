#!/usr/bin/env bash
# UI verification only: no compiler or solver is launched on the desktop host.
# UI 验证仅在构建虚拟机执行，并为本次进程树设置资源上限。
set -euo pipefail
if (( $# == 0 )); then
    echo 'Usage: scripts/verify-in-vm.sh <command> [args...]' >&2
    exit 2
fi
ui_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# Lightweight source/configuration admission only; all compilation stays inside VM.
python3 "$ui_root/scripts/check-blitz-native-dependencies.py" >/dev/null
exec 9>/tmp/jasper-ui-rust-verification.lock
flock -n 9 || { echo 'Another UI verification run is active.' >&2; exit 75; }
# Cold dependency compilation can need longer than an individual proof workload.
# Only wall time is configurable; memory, swap, tasks and job limits stay fixed.
ui_work_seconds=${JASPER_UI_VM_TIMEOUT_SECONDS:-180}
case "$ui_work_seconds" in
    ''|*[!0-9]*) echo 'UI VM timeout must be an integer from 1 to 900.' >&2; exit 2 ;;
esac
if (( ${#ui_work_seconds} > 3 )); then
    echo 'UI VM timeout must be an integer from 1 to 900.' >&2
    exit 2
fi
ui_work_seconds=$((10#$ui_work_seconds))
if (( ui_work_seconds < 1 || ui_work_seconds > 900 )); then
    echo 'UI VM timeout must be an integer from 1 to 900.' >&2
    exit 2
fi
ui_transport_seconds=$((ui_work_seconds + 120))
timeout --kill-after=10s "${ui_transport_seconds}s" env VM_NICE=15 VM_KEEP_AWAKE=0 bash /home/auth/Documents/jasper/jasper-os-build/vm-run.sh \
    "$ui_root" systemd-run --user --scope --quiet --expand-environment=no \
    --property=MemoryMax=3G --property=MemorySwapMax=0 --property=TasksMax=128 \
    env CARGO_BUILD_JOBS=1 JASPER_UI_VERIFICATION_VM=1 \
    timeout --kill-after=10s "${ui_work_seconds}s" "$@"
