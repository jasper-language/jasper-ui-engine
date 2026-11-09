#!/usr/bin/env bash
# Build the iOS/iPadOS reference host .app bundle on a macOS host with Xcode.
# 在装有 Xcode 的 macOS 构建机上产出 iOS/iPadOS 参考宿主 .app。
#
# Usage: bash ios/build-app.sh [simulator|device] [--sign IDENTITY] [--debug]
#
# simulator: aarch64-apple-ios-sim, ad-hoc signed, installable in an installed Simulator runtime.
# device:    aarch64-apple-ios, signed only when a code-signing identity is available.
set -euo pipefail

ios_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$ios_root/.." && pwd)

mode=${1:-simulator}
shift || true
identity=""
profile=release
while [[ $# -gt 0 ]]; do
    case "$1" in
        --sign) identity=$2; shift 2 ;;
        --debug) profile=debug; shift ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

case "$mode" in
    simulator)
        target=aarch64-apple-ios-sim
        platform=iPhoneSimulator
        dt_platform=iphonesimulator
        linker_platform=ios-simulator
        ;;
    device)
        target=aarch64-apple-ios
        platform=iPhoneOS
        dt_platform=iphoneos
        linker_platform=ios
        ;;
    *)
        echo "usage: bash ios/build-app.sh [simulator|device] [--sign IDENTITY]" >&2
        exit 2
        ;;
esac

command -v xcrun >/dev/null
command -v plutil >/dev/null
command -v codesign >/dev/null
command -v vtool >/dev/null

sdk_version=$(xcrun --sdk "$dt_platform" --show-sdk-version)

cd "$ios_root"
# Emit a modern LC_BUILD_VERSION so the executable declares its iOS platform explicitly.
# rustc's Apple target still passes the legacy -mios-version-min; ld resolves both to 15.0.
# 显式产出 LC_BUILD_VERSION，让可执行文件声明自己的 iOS 平台；ld 会把两个最低版本都归一到 15.0。
export RUSTFLAGS="-C link-arg=-Wl,-platform_version -C link-arg=-Wl,${linker_platform} -C link-arg=-Wl,15.0 -C link-arg=-Wl,15.0"
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_INCREMENTAL=0

if [[ "$profile" == release ]]; then
    cargo build --locked --release --target "$target"
else
    cargo build --locked --target "$target"
fi

target_dir=${CARGO_TARGET_DIR:-$ios_root/target}
out="$ios_root/target/app/$mode"
app="$out/JasperUI.app"
rm -rf "$app"
mkdir -p "$app"

cp "$target_dir/$target/$profile/jasper-ui" "$app/jasper-ui"
printf 'APPL????' > "$app/PkgInfo"
sed -e "s|@PLATFORM@|$platform|" \
    -e "s|@DT_PLATFORM@|$dt_platform|" \
    -e "s|@SDK@|${dt_platform}${sdk_version}|" \
    "$ios_root/Info.plist.in" > "$app/Info.plist"

plutil -lint "$app/Info.plist"

if [[ "$mode" == simulator ]]; then
    codesign --force --sign - --timestamp=none --identifier org.jasper.ui.ios "$app"
    signature="ad-hoc"
elif [[ -n "$identity" ]]; then
    codesign --force --sign "$identity" --timestamp=none --identifier org.jasper.ui.ios "$app"
    signature="$identity"
else
    discovered=$(security find-identity -v -p codesigning 2>/dev/null | awk '/"/{print $2; exit}')
    if [[ -n "$discovered" ]]; then
        codesign --force --sign "$discovered" --timestamp=none --identifier org.jasper.ui.ios "$app"
        signature="$discovered"
    else
        signature="unavailable"
        echo "device_signing=unavailable (no code-signing identity; install requires a signed build)" >&2
    fi
fi

echo "ios_app=$app"
echo "ios_target=$target"
echo "ios_platform=$platform"
echo "ios_min_version=15.0"
echo "ios_sdk=$sdk_version"
echo "ios_signature=$signature"
vtool -show-build "$app/jasper-ui" | sed -n '1,10p'
shasum -a 256 "$app/jasper-ui" "$app/Info.plist"
if [[ "$signature" != unavailable ]]; then
    codesign --verify --deep --strict --verbose=2 "$app"
fi
