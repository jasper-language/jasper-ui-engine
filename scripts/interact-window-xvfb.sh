#!/usr/bin/env bash
# Verify that a real X11 pointer click crosses the public window input path.
set -euo pipefail
if (( $# != 2 )); then
    echo "usage: $0 FONT OUTPUT_DIR" >&2
    exit 2
fi
ui_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
font=$1
out=$2
mkdir -p "$out"
display=:${JASPER_UI_XVFB_DISPLAY:-100}
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
cd "$ui_root"
cargo run --locked --features desktop,native-engine-cpu \
    --example window -- "$font" --once-after-activation >"$out/interaction.log" 2>&1 &
app_pid=$!
window_id=
for _ in $(seq 1 480); do
    window_id=$(xdotool search --name '^Jasper UI Engine$' 2>/dev/null | head -n 1 || true)
    [[ -n "$window_id" ]] && break
    sleep 0.25
done
test -n "$window_id"
# The public scene places the button at logical (32,32), size 200x52.
xdotool mousemove --window "$window_id" 100 50 click 1
wait "$app_pid"
app_pid=
grep -q 'item5_window=pass activated=1' "$out/interaction.log"
