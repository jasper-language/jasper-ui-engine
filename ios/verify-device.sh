#!/usr/bin/env bash
# Verify the iOS/iPadOS reference host on connected devices.
# 在已连接的真机上验证 iOS/iPadOS 参考宿主。
#
# For every paired device: build a signed app through Xcode, install it with devicectl, launch it,
# read the frame the app presented from the app container, and optionally capture an Apple Pencil
# session. Device names, UDIDs, certificates and team identifiers are never printed.
# 对每台已配对设备：用 Xcode 构建签名 app、devicectl 安装、启动、从 app 容器取回呈现帧，
# 并可捕获一次 Apple Pencil 会话。不打印设备名、UDID、证书或团队标识。
#
# Usage: bash ios/verify-device.sh [--team TEAM] [--out DIR] [--pencil SECONDS] [--device UDID]
set -euo pipefail

ios_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$ios_root/.." && pwd)

team=${JASPER_TEAM:-}
out=${JASPER_DEVICE_OUT:-$repo_root/evidence/ios-device}
pencil_seconds=0
only_device=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --team) team=$2; shift 2 ;;
        --out) out=$2; shift 2 ;;
        --pencil) pencil_seconds=$2; shift 2 ;;
        --device) only_device=$2; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
mkdir -p "$out"

redact() {
    sed -E \
        -e 's/[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}/<device-id>/g' \
        -e 's/[0-9A-Fa-f]{8}-[0-9A-Fa-f]{16}/<device-id>/g' \
        -e 's/[0-9A-Fa-f]{40}/<id>/g' \
        -e 's/Apple Development: [^,]*/Apple Development: <redacted>/g' \
        -e 's/JasperUI\.app[^ ]*//g'
}

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "device_verify=skipped reason=not_macos"
    exit 3
fi

identity=$(security find-identity -v -p codesigning 2>/dev/null | awk '/"/{print $2; exit}')
if [[ -z "$identity" ]]; then
    echo "device_verify=blocked reason=no_signing_identity"
    exit 3
fi
if [[ -z "$team" ]]; then
    team=$(printf '%s' "$identity" | sed -n 's/.*(\([A-Z0-9]\{10\}\))$/\1/p')
fi
if [[ -z "$team" ]]; then
    echo "device_verify=blocked reason=no_team hint=set_JASPER_TEAM"
    exit 3
fi

echo "device_verify_identity=present"
echo "device_verify_team=present"

mkdir -p "$ios_root/app/DerivedData"
if ! bash "$ios_root/build-xcode.sh" device --team "$team" > "$out/xcode-build.log" 2>&1; then
    echo "device_verify=blocked reason=xcode_build_failed"
    redact < "$out/xcode-build.log" | tail -15
    exit 1
fi
app="$ios_root/app/DerivedData/Build/Products/Release-iphoneos/JasperUI.app"
echo "device_verify_app=$app"

# Enumerate paired devices without printing their identifiers.
devices_json="$out/devices.json"
xcrun devicectl list devices --json-output "$devices_json" >/dev/null 2>&1
mapfile -t devices < <(python3 - "$devices_json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
for device in data.get("result", {}).get("devices", []):
    state = device.get("connectionProperties", {}).get("pairingState", "")
    if state != "paired":
        continue
    print(device["hardwareProperties"]["udid"])
PY
)

if [[ ${#devices[@]} -eq 0 ]]; then
    echo "device_verify=blocked reason=no_paired_device"
    exit 3
fi
echo "device_verify_devices=${#devices[@]}"

index=0
failures=0
for udid in "${devices[@]}"; do
    index=$((index + 1))
    if [[ -n "$only_device" && "$udid" != "$only_device" ]]; then
        continue
    fi
    log="$out/device-$index.log"
    echo "device_verify[$index]=install"
    if ! xcrun devicectl device install app --device "$udid" "$app" 2>&1 | redact | tee "$out/device-$index-install.log" | tail -3; then
        echo "device_verify[$index]=fail reason=install"
        failures=$((failures + 1))
        continue
    fi

    echo "device_verify[$index]=launch"
    set +e
    xcrun devicectl device process launch --device "$udid" --console --terminate-existing \
        org.jasper.ui.ios --frames 2 --tap center --exit-after-ms 15000 2>&1 \
        | redact | tee "$log" | tail -20
    set -e

    if grep -q "ios_host=pass" "$log"; then
        echo "device_verify[$index]=pass"
    else
        echo "device_verify[$index]=fail reason=run"
        failures=$((failures + 1))
    fi

    remote_frame=$(sed -n 's/.*readback path=\([^ ]*frame\.ppm\).*/\1/p' "$log" | tail -1)
    if [[ -n "$remote_frame" ]]; then
        if xcrun devicectl device copy from --device "$udid" \
            --source "$remote_frame" --destination "$out/device-$index-frame.ppm" >/dev/null 2>&1; then
            echo "device_verify[$index]_frame_sha256=$(shasum -a 256 "$out/device-$index-frame.ppm" | cut -d' ' -f1)"
        else
            echo "device_verify[$index]_frame=pull_failed"
        fi
    else
        echo "device_verify[$index]_frame=not_reported"
    fi

    if [[ "$pencil_seconds" -gt 0 ]]; then
        echo "device_verify[$index]=pencil_prompt 现在用 Apple Pencil 在屏幕上画 ${pencil_seconds}s"
        set +e
        xcrun devicectl device process launch --device "$udid" --console --terminate-existing \
            org.jasper.ui.ios --exit-after-ms "$((pencil_seconds * 1000))" 2>&1 \
            | redact | tee "$out/device-$index-pencil.log" | grep -E "stage=pencil" | tail -12
        set -e
        pencils=$(grep -c "stage=pencil" "$out/device-$index-pencil.log" || true)
        echo "device_verify[$index]_pencil_events=${pencils:-0}"
        sed -n 's/.*pressure=\([0-9.]*\).*/pencil_pressure_sample=\1/p' \
            "$out/device-$index-pencil.log" | head -5
    fi
done

if [[ "$failures" -ne 0 ]]; then
    echo "device_verify=fail failures=$failures"
    exit 1
fi
echo "device_verify=pass"
