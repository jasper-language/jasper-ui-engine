#!/usr/bin/env bash
# Build the Jasper UI iOS app through Xcode so that signing, the provisioning profile and the
# device deployment are handled by Xcode itself.
# 通过 Xcode 构建 Jasper UI iOS app，让签名、描述文件与真机部署由 Xcode 负责。
#
# The Rust host is compiled as a static library by an Xcode build phase; `main.m` calls
# `jasper_ios_main`, which hands control to winit and `UIApplicationMain`.
# Rust 宿主由 Xcode 构建阶段编成静态库；`main.m` 调用 `jasper_ios_main`，后者把控制权交给 winit
# 与 `UIApplicationMain`。
#
# Requirements: Xcode's iOS platform component installed (Xcode -> Settings -> Components -> iOS),
# plus a development team for device builds. Simulator builds need no signing.
# 前提：已安装 Xcode 的 iOS 平台组件（Xcode → Settings → Components → iOS）；真机还需要开发团队。
# 模拟器构建不需要签名。
#
# Usage: bash ios/build-xcode.sh [device|simulator] [--team TEAM_ID]
#        JASPER_TEAM=... JASPER_CARGO=/path/to/cargo bash ios/build-xcode.sh device
set -euo pipefail

ios_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
mode=${1:-device}
shift || true
team=${JASPER_TEAM:-}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --team) team=$2; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "xcode_build=skipped reason=not_macos"
    exit 3
fi

case "$mode" in
    device)
        sdk=iphoneos
        destination='generic/platform=iOS'
        product='Release-iphoneos'
        ;;
    simulator)
        sdk=iphonesimulator
        destination='generic/platform=iOS Simulator'
        product='Release-iphonesimulator'
        ;;
    *)
        echo "usage: bash ios/build-xcode.sh [device|simulator] [--team TEAM_ID]" >&2
        exit 2
        ;;
esac

cargo_bin=${JASPER_CARGO:-$(command -v cargo || true)}
if [[ -z "$cargo_bin" ]]; then
    cargo_bin="$HOME/.cargo/bin/cargo"
fi
if [[ ! -x "$cargo_bin" ]]; then
    echo "xcode_build=fail reason=no_cargo hint=set_JASPER_CARGO"
    exit 3
fi

# Xcode build phases inherit the environment, so pin the toolchain homes next to the cargo binary
# (a workspace-local toolchain has no ~/.rustup and would otherwise fail to create its home).
# Xcode 构建阶段继承环境；把工具链 home 固定到 cargo 旁边的目录（工作区内的工具链没有 ~/.rustup）。
export CARGO_HOME="${CARGO_HOME:-$(cd "$(dirname "$cargo_bin")/.." && pwd)}"
if [[ -d "$CARGO_HOME/../rustup" ]]; then
    export RUSTUP_HOME="${RUSTUP_HOME:-$(cd "$CARGO_HOME/../rustup" && pwd)}"
fi

derived="$ios_root/app/DerivedData"
mkdir -p "$derived"

echo "xcode_build_mode=$mode"
# macOS ships bash 3.2, where an empty array expansion trips `set -u`; branch instead.
# macOS 自带 bash 3.2，空数组展开会触发 `set -u`，因此分支调用。
if [[ -n "$team" ]]; then
    JASPER_CARGO="$cargo_bin" xcodebuild \
        -project "$ios_root/app/JasperUI.xcodeproj" \
        -scheme JasperUI \
        -configuration Release \
        -sdk "$sdk" \
        -destination "$destination" \
        -derivedDataPath "$derived" \
        -allowProvisioningUpdates \
        DEVELOPMENT_TEAM="$team" \
        ARCHS=arm64 \
        build
else
    JASPER_CARGO="$cargo_bin" xcodebuild \
        -project "$ios_root/app/JasperUI.xcodeproj" \
        -scheme JasperUI \
        -configuration Release \
        -sdk "$sdk" \
        -destination "$destination" \
        -derivedDataPath "$derived" \
        -allowProvisioningUpdates \
        ARCHS=arm64 \
        build
fi

app="$derived/Build/Products/$product/JasperUI.app"
if [[ ! -d "$app" ]]; then
    echo "xcode_build=fail reason=app_missing path=$app"
    exit 1
fi
echo "xcode_app=$app"
if [[ "$mode" == device ]]; then
    codesign -dv "$app" 2>&1 | sed -n 's/^Authority=/xcode_authority=/p' | head -2
    codesign --verify --deep --strict "$app" && echo "xcode_signature=valid"
fi
