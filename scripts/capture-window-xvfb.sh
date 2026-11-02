#!/usr/bin/env bash
# Capture the host-window presentation through Xvfb for VM-only diagnostics.
set -euo pipefail
if (( $# != 2 )); then
    echo "usage: $0 FONT OUTPUT_DIR" >&2
    exit 2
fi
font=$1
out=$2
mkdir -p "$out"
display=:${JASPER_UI_XVFB_DISPLAY:-99}
Xvfb "$display" -screen 0 1280x720x24 -nolisten tcp >"$out/xvfb.log" 2>&1 &
xvfb_pid=$!
app_pid=
cleanup() {
    if [[ -n "$app_pid" ]]; then
        kill "$app_pid" 2>/dev/null || true
        wait "$app_pid" 2>/dev/null || true
    fi
    kill "$xvfb_pid" 2>/dev/null || true
    wait "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT
export DISPLAY=$display
sleep 1
cargo run --locked --features desktop,native-engine-cpu --example window -- "$font" >"$out/window.log" 2>&1 &
app_pid=$!
window_id=
for _ in $(seq 1 300); do
    kill -0 "$app_pid"
    window_id=$(xdotool search --name '^Jasper UI Engine$' 2>/dev/null | head -n 1 || true)
    if [[ -n "$window_id" ]]; then
        break
    fi
    sleep 1
done
test -n "$window_id"
xwd -root -silent -out "$out/root.xwd"
xwdtopnm "$out/root.xwd" >"$out/root.ppm"
xdotool windowsize "$window_id" 640 360
sleep 2
xwd -root -silent -out "$out/resized.xwd"
xwdtopnm "$out/resized.xwd" >"$out/resized.ppm"

# Exercise the host-owned Vulkan Surface/TextureView/present path on the same X11 endpoint.
kill "$app_pid" 2>/dev/null || true
wait "$app_pid" 2>/dev/null || true
app_pid=
cargo run --locked --features desktop,native-blitz-cpu-pending --example window -- "$font" --once \
    >"$out/window-once.log" 2>&1
cargo run --locked --features desktop,gpu-pending --example gpu_window -- "$font" --once \
    >"$out/gpu-window-once.log" 2>&1
