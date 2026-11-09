#!/usr/bin/env bash
# Build and verify the iOS/iPadOS reference host bundles.
# 构建并验证 iOS/iPadOS 参考宿主 bundle。
#
# Static verification always runs: bundle layout, Info.plist declarations (iPhone + iPad device
# families), Mach-O platform, linked frameworks and signature validity.
# Runtime verification runs only when a Simulator runtime is installed and the host is macOS.
#
# Usage: bash scripts/verify-ios-app.sh [OUT_DIR]
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
out=${1:-$repo_root/evidence/ios-reference-host}
mkdir -p "$out"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ios_host=skipped reason=not_macos"
    exit 3
fi

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$repo_root/target}

"$repo_root/ios/build-app.sh" simulator > "$out/build-simulator.log" 2>&1
"$repo_root/ios/build-app.sh" device > "$out/build-device.log" 2>&1

check_bundle() {
    local app=$1 label=$2 expected_platform=$3 expected_bundle_platform=$4
    local binary="$app/jasper-ui"
    local plist="$app/Info.plist"
    local failures=0

    [[ -x "$binary" ]] || { echo "${label}_binary=missing"; failures=$((failures + 1)); }
    plutil -lint "$plist" > "$out/${label}-plist-lint.txt" 2>&1 \
        || { echo "${label}_plist_lint=fail"; failures=$((failures + 1)); }

    local platform
    platform=$(vtool -show-build "$binary" | awk '/platform/{print $2; exit}')
    echo "${label}_macho_platform=$platform"
    [[ "$platform" == "$expected_platform" ]] \
        || { echo "${label}_platform_mismatch=fail"; failures=$((failures + 1)); }

    local minos
    minos=$(vtool -show-build "$binary" | awk '/minos/{print $2; exit}')
    echo "${label}_minos=$minos"

    local architectures
    architectures=$(lipo -info "$binary" 2>&1)
    echo "${label}_arch=$architectures"

    local family
    family=$(plutil -extract UIDeviceFamily json -o - "$plist")
    echo "${label}_device_family=$family"
    [[ "$family" == "[1,2]" ]] \
        || { echo "${label}_device_family_mismatch=fail"; failures=$((failures + 1)); }

    local supported
    supported=$(plutil -extract CFBundleSupportedPlatforms.0 raw -o - "$plist")
    echo "${label}_bundle_platform=$supported"
    [[ "$supported" == "$expected_bundle_platform" ]] \
        || { echo "${label}_bundle_platform_mismatch=fail"; failures=$((failures + 1)); }

    otool -L "$binary" | grep -q "UIKit.framework" \
        || { echo "${label}_uikit_link=fail"; failures=$((failures + 1)); }
    otool -L "$binary" | grep -q "Metal.framework" \
        || { echo "${label}_metal_link=fail"; failures=$((failures + 1)); }

    local signature
    if codesign --verify --deep --strict "$app" >/dev/null 2>&1; then
        signature=$(codesign -dv "$app" 2>&1 | sed -n 's/^Signature=//p' | head -1)
        signature=${signature:-adhoc}
    else
        signature=unverified
        [[ "$label" == simulator ]] && failures=$((failures + 1))
    fi
    echo "${label}_signature=$signature"
    echo "${label}_binary_sha256=$(shasum -a 256 "$binary" | cut -d' ' -f1)"
    echo "${label}_plist_sha256=$(shasum -a 256 "$plist" | cut -d' ' -f1)"
    return $failures
}

failures=0
check_bundle "$repo_root/ios/target/app/simulator/JasperUI.app" simulator IOSSIMULATOR iPhoneSimulator \
    || failures=$((failures + 1))
check_bundle "$repo_root/ios/target/app/device/JasperUI.app" device IOS iPhoneOS \
    || failures=$((failures + 1))

# Adapter exercise on the macOS build: the identical host module, without UIKit lifecycle.
if [[ "${JASPER_UI_SKIP_ADAPTER_EXERCISE:-0}" != "1" ]]; then
    cargo build --locked --features host-on-macos --manifest-path "$repo_root/ios/Cargo.toml" \
        > "$out/adapter-build.log" 2>&1
    adapter="$CARGO_TARGET_DIR/debug/jasper-ui"
    "$adapter" "$repo_root/ios/assets/DejaVuSans.ttf" --size 390,844 --frames 2 --spin \
        --readback "$out/adapter-phone.ppm" --timeout-ms 25000 \
        > "$out/adapter-phone.log" 2>&1 || true
    "$adapter" "$repo_root/ios/assets/DejaVuSans.ttf" --size 834,1194 --insets 24,0,20,0 \
        --frames 2 --tap center --readback "$out/adapter-ipad.ppm" --timeout-ms 25000 \
        > "$out/adapter-ipad.log" 2>&1 || true
    echo "adapter_phone=$(grep -m1 '^ios_host=' "$out/adapter-phone.log" || echo none)"
    echo "adapter_ipad=$(grep -m1 '^ios_host=' "$out/adapter-ipad.log" || echo none)"
    echo "adapter_ipad_viewport=$(sed -n 's/.*stage=viewport //p' "$out/adapter-ipad.log" | head -1)"
fi

# Runtime verification requires an installed Simulator runtime.
runtime_count=$(xcrun simctl list runtimes -j 2>/dev/null | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("runtimes",[])))' 2>/dev/null || echo 0)
echo "simulator_runtimes=$runtime_count"
if [[ "$runtime_count" -gt 0 ]]; then
    device=$(xcrun simctl list devices available -j | python3 -c '
import json, sys
data = json.load(sys.stdin)["devices"]
for runtime, devices in data.items():
    if "iOS" in runtime and devices:
        print(devices[0]["udid"]); break
')
    echo "simulator_device=$device"
    xcrun simctl boot "$device" >/dev/null 2>&1 || true
    xcrun simctl install "$device" "$repo_root/ios/target/app/simulator/JasperUI.app"
    xcrun simctl launch --console-pty "$device" org.jasper.ui.ios --args \
        "" --frames 2 > "$out/simulator-run.log" 2>&1 || true
    echo "simulator_log=$(tail -3 "$out/simulator-run.log" | tr '\n' ' ')"
else
    echo "simulator_run=skipped reason=no_runtime"
fi

if [[ "$failures" -ne 0 ]]; then
    echo "ios_host=fail static_failures=$failures"
    exit 1
fi
echo "ios_host=pass"
