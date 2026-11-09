#!/usr/bin/env bash
# Install and launch the iOS/iPadOS reference host on a connected device, then pull the frame
# the app presented and verify its digest.
# 在已连接的真机上安装并启动 iOS/iPadOS 参考宿主，取回它呈现的帧并校验摘要。
#
# Requirements on the Mac:
# - the device is connected (USB or paired Wi-Fi) and trusted;
# - a code-signing identity exists in the login keychain (Xcode -> Settings -> Accounts ->
#   add your Apple ID, then a development certificate is created on the first device build);
# - a provisioning profile for `org.jasper.ui.ios` that includes the device, passed with
#   --profile (Xcode writes them to ~/Library/MobileDevice/Provisioning Profiles).
#
# Usage: bash ios/install-device.sh [--device UDID|NAME] [--identity NAME] [--profile PATH]
#                                    [--out DIR]
set -euo pipefail

ios_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$ios_root/.." && pwd)

device=""
identity=""
profile=""
out="$repo_root/evidence/ios-device"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --device) device=$2; shift 2 ;;
        --identity) identity=$2; shift 2 ;;
        --profile) profile=$2; shift 2 ;;
        --out) out=$2; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
mkdir -p "$out"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "device_install=skipped reason=not_macos"
    exit 3
fi

if [[ -z "$identity" ]]; then
    identity=$(security find-identity -v -p codesigning 2>/dev/null | awk '/"/{print $2; exit}')
fi
if [[ -z "$identity" ]]; then
    cat >&2 <<'EOF'
device_install=blocked reason=no_signing_identity
No code-signing identity is present, so an installable .app cannot be produced.
One-time setup on this Mac:
  1. Open Xcode -> Settings -> Accounts and add your Apple ID (a free account works).
  2. Select the team, then let Xcode create a development certificate (build any template app
     once for a connected device), or pass --identity with an existing one.
  3. Pass --profile with the provisioning profile for org.jasper.ui.ios that includes this device.
Also required on iOS 16+: Settings -> Privacy & Security -> Developer Mode, then reboot the device.
EOF
    exit 3
fi

if [[ -z "$device" ]]; then
    device=$(xcrun devicectl list devices 2>/dev/null | awk '/available/{print $3; exit}')
fi
if [[ -z "$device" ]]; then
    echo "device_install=blocked reason=no_available_device"
    xcrun devicectl list devices 2>&1 | tail -3
    exit 3
fi

# Never echo certificate names, team identifiers or UDIDs into logs that may be published.
# 证书名、团队标识与 UDID 不写入可能被公开的日志。
echo "device_install_identity=present"
echo "device_install_device=redacted"

# Xcode builds, signs and provisions the app; the manual bundle path cannot create a profile.
# Xcode 负责构建、签名与描述文件；手工 bundle 路径无法生成描述文件。
team=${JASPER_TEAM:-$(printf '%s' "$identity" | sed -n 's/.*(\([A-Z0-9]\{10\}\))$/\1/p')}
if [[ -z "$team" ]]; then
    echo "device_install=blocked reason=no_team hint=JASPER_TEAM_or_Xcode_Manage_Certificates"
    exit 3
fi
bash "$ios_root/build-xcode.sh" device --team "$team" > "$out/xcode-build.log" 2>&1 \
    || { echo "device_install=blocked reason=xcode_build_failed"; tail -20 "$out/xcode-build.log"; exit 1; }
app="$ios_root/app/DerivedData/Build/Products/Release-iphoneos/JasperUI.app"
if [[ ! -d "$app" ]]; then
    echo "device_install=blocked reason=app_missing"
    exit 1
fi

xcrun devicectl device install app --device "$device" "$app" 2>&1 \
    | sed -E 's/[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}/<device-id>/g' \
    | tee "$out/install.log" | tail -5

container=$(xcrun devicectl device info files --device "$device" \
    --domain-type appDataContainer --domain-identifier org.jasper.ui.ios 2>/dev/null \
    | awk -F': ' '/appDataContainer/{print $2; exit}')
if [[ -z "$container" ]]; then
    container=$(xcrun devicectl device info files --device "$device" \
        --domain-type appDataContainer --domain-identifier org.jasper.ui.ios 2>/dev/null \
        | sed -n 's/.*path *= *//p' | head -1)
fi
echo "device_container=appDataContainer"

frame_remote=""
if [[ -n "$container" ]]; then
    frame_remote="$container/Documents/frame.ppm"
fi

xcrun devicectl device process launch --device "$device" --console --terminate-existing \
    org.jasper.ui.ios --frames 2 --tap center ${frame_remote:+--readback "$frame_remote"} \
    2>&1 | tee "$out/launch.log" | tail -20

if [[ -n "$frame_remote" ]]; then
    if xcrun devicectl device copy from --device "$device" \
        --source "$frame_remote" --destination "$out/frame.ppm" >/dev/null 2>&1; then
        echo "device_frame=$out/frame.ppm"
        shasum -a 256 "$out/frame.ppm"
    else
        echo "device_frame=pull_failed source=$frame_remote"
    fi
fi

if grep -q "ios_host=pass" "$out/launch.log"; then
    echo "device_run=pass"
else
    echo "device_run=fail"
    exit 1
fi
