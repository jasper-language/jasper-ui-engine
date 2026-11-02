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
cargo run --locked --features desktop,native-blitz-cpu-pending --example window -- "$font" >"$out/window.log" 2>&1 &
app_pid=$!
sleep 4
kill -0 "$app_pid"
xwd -root -silent -out "$out/root.xwd"
xwdtopnm "$out/root.xwd" >"$out/root.ppm"
window_id=$(xdotool search --name '^Jasper UI Engine$' | head -n 1)
test -n "$window_id"
xdotool windowsize "$window_id" 640 360
sleep 2
xwd -root -silent -out "$out/resized.xwd"
xwdtopnm "$out/resized.xwd" >"$out/resized.ppm"
