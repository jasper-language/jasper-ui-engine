#!/usr/bin/env bash
# Verify the macOS reference host on a real macOS desktop session.
# 在真实 macOS 桌面会话中验证 macOS 参考宿主。
#
# What it proves, in order:
# 1. a Metal surface frame is presented and read back from the drawable;
# 2. the AccessKit Cocoa adapter exposes the scene button through the real AX API;
# 3. an AXPress activation and a real WindowServer pointer click both reach the engine and
#    produce a versioned repaint;
# 4. a CJK font injection renders Chinese glyphs instead of tofu;
# 5. the view retires and the process exits cleanly.
#
# Usage: bash scripts/verify-macos-host.sh [OUT_DIR] [--binary PATH] [--no-build]
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
out=${1:-$repo_root/evidence/macos-reference-host}
shift || true
binary=${JASPER_UI_MACOS_BIN:-}
build=true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary) binary=$2; shift 2 ;;
        --no-build) build=false; shift ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "macos_host=skipped reason=not_macos"
    exit 3
fi
command -v swiftc >/dev/null || { echo "macos_host=skipped reason=no_swiftc"; exit 3; }

mkdir -p "$out"

font=${JASPER_UI_TEST_FONT:-$repo_root/macos/assets/DejaVuSans.ttf}
# The host bundles Android's ordered Latin+CJK stack; the control run drops the CJK face.
# 宿主自带与 Android 相同的有序拉丁+CJK 字体栈；对照运行去掉 CJK 字面。


if [[ "$build" == true && -z "$binary" ]]; then
    cargo build --locked --manifest-path "$repo_root/macos/Cargo.toml"
fi
if [[ -z "$binary" ]]; then
    target_dir=${CARGO_TARGET_DIR:-$repo_root/macos/target}
    binary="$target_dir/debug/jasper-ui"
fi
[[ -x "$binary" ]] || { echo "macos_host=fail reason=missing_binary binary=$binary"; exit 1; }

swiftc -O "$repo_root/macos/tools/ax-probe.swift" -o "$out/ax-probe"
swiftc -O "$repo_root/macos/tools/os-click.swift" -o "$out/os-click"

wait_for_line() {
    local file=$1 pattern=$2 timeout=${3:-30} counter=0
    while (( counter < timeout * 10 )); do
        if grep -q "$pattern" "$file" 2>/dev/null; then
            return 0
        fi
        sleep 0.1
        counter=$((counter + 1))
    done
    return 1
}

session_log="$out/session.log"
rm -f "$session_log"
"$binary" "$font" --exit-after-ms 12000 \
    --readback "$out/latest.ppm" \
    >"$session_log" 2>&1 &
app_pid=$!
trap 'kill -9 "$app_pid" 2>/dev/null || true' EXIT

if ! wait_for_line "$session_log" "stage=window created" 30; then
    echo "macos_host=fail reason=window_not_created"
    cat "$session_log"
    exit 1
fi
if ! wait_for_line "$session_log" "stage=frame index=1" 30; then
    echo "macos_host=fail reason=first_frame_missing"
    cat "$session_log"
    exit 1
fi
pid=$(sed -n 's/.*stage=window created pid=\([0-9]*\).*/\1/p' "$session_log" | head -1)

# The first AX traversal activates the Cocoa bridge; the second one reads the populated tree.
"$out/ax-probe" "$pid" > "$out/ax-initial.txt" 2>&1 || true
sleep 1
"$out/ax-probe" "$pid" --press Activate > "$out/ax-press.txt" 2>&1 || true

read -r ax_x ax_y ax_w ax_h < <(
    sed -n 's/.*role=AXButton title="[^"]*Activate".*position=\([0-9-]*\),\([0-9-]*\) size=\([0-9]*\)x\([0-9]*\).*/\1 \2 \3 \4/p' \
        "$out/ax-press.txt" | head -1
)
if [[ -z "${ax_x:-}" ]]; then
    echo "macos_host=fail reason=ax_button_missing"
    cat "$out/ax-press.txt"
    exit 1
fi
click_x=$((ax_x + ax_w / 2))
click_y=$((ax_y + ax_h / 2))
"$out/os-click" "$pid" "$click_x" "$click_y" > "$out/os-click.txt" 2>&1 || true

if ! wait_for_line "$session_log" "macos_host=" 20; then
    echo "macos_host=fail reason=session_did_not_finish"
    tail -20 "$session_log"
    exit 1
fi
wait "$app_pid" || true
trap - EXIT

activations=$(sed -n 's/.*stage=activate count=\([0-9]*\).*/\1/p' "$session_log" | tail -1)
frames=$(sed -n 's/.*stage=frame index=\([0-9]*\).*/\1/p' "$session_log" | tail -1)
summary=$(grep -m1 "^macos_host=" "$session_log" || true)
ax_press=$(grep -m1 "^ax_press " "$out/ax-press.txt" || true)
os_click=$(grep -m1 "^os_click=posted" "$out/os-click.txt" || true)

# CJK proof: the bundled Latin+CJK stack versus the same line without the CJK face.
"$binary" "$font" --frames 1 --spin --readback "$out/cjk.ppm" --timeout-ms 20000 \
    > "$out/cjk.log" 2>&1 || true
"$binary" "$font" --cjk-tofu --frames 1 --spin --readback "$out/tofu.ppm" --timeout-ms 20000 \
    > "$out/tofu.log" 2>&1 || true
cjk_other=$(sed -n 's/.*other=\([0-9]*\).*/\1/p' "$out/cjk.log" | tail -1)
tofu_other=$(sed -n 's/.*other=\([0-9]*\).*/\1/p' "$out/tofu.log" | tail -1)
cjk_sha=$(shasum -a 256 "$out/cjk.ppm" 2>/dev/null | cut -d' ' -f1)
tofu_sha=$(shasum -a 256 "$out/tofu.ppm" 2>/dev/null | cut -d' ' -f1)

echo "macos_host=$([[ $summary == *"macos_host=pass"* ]] && echo pass || echo fail)"
echo "summary=$summary"
echo "frames=${frames:-0}"
echo "activations=${activations:-0}"
echo "ax_button=${ax_x},${ax_y} ${ax_w}x${ax_h}"
echo "ax_press=${ax_press:-none}"
echo "os_click=${os_click:-none}"
echo "cjk_ink=${cjk_other:-0}"
echo "tofu_ink=${tofu_other:-0}"
echo "cjk_frame_sha256=${cjk_sha:-none}"
echo "tofu_frame_sha256=${tofu_sha:-none}"

if [[ "$summary" != *"macos_host=pass"* ]]; then
    exit 1
fi
if [[ "${activations:-0}" -lt 2 ]]; then
    echo "macos_host=fail reason=activation_round_trip_incomplete"
    exit 1
fi
if [[ "$cjk_sha" == "$tofu_sha" ]]; then
    echo "macos_host=fail reason=cjk_injection_made_no_difference"
    exit 1
fi
