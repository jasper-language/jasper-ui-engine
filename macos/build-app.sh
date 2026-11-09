#!/usr/bin/env bash
# Build the macOS reference host .app bundle.
# 构建 macOS 参考宿主 .app。
#
# Usage: bash macos/build-app.sh [--debug] [--universal] [--sign IDENTITY]
#
# The default build is a native (arm64) release bundle with an ad-hoc signature, which is enough
# to launch locally. `--universal` also builds x86_64 and merges both slices with `lipo`.
set -euo pipefail

macos_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$macos_root/.." && pwd)

profile=release
universal=false
identity="-"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug) profile=debug; shift ;;
        --universal) universal=true; shift ;;
        --sign) identity=$2; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

command -v codesign >/dev/null
command -v plutil >/dev/null
command -v shasum >/dev/null

export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_INCREMENTAL=0

cd "$macos_root"
target_dir=${CARGO_TARGET_DIR:-$macos_root/target}

build() {
    local target=$1
    if [[ "$profile" == release ]]; then
        cargo build --locked --release --target "$target"
    else
        cargo build --locked --target "$target"
    fi
}

build aarch64-apple-darwin
binary="$target_dir/aarch64-apple-darwin/$profile/jasper-ui"
if [[ "$universal" == true ]]; then
    build x86_64-apple-darwin
    lipo -create \
        "$target_dir/aarch64-apple-darwin/$profile/jasper-ui" \
        "$target_dir/x86_64-apple-darwin/$profile/jasper-ui" \
        -output "$target_dir/jasper-ui-universal"
    binary="$target_dir/jasper-ui-universal"
fi

out="$macos_root/target/app"
app="$out/Jasper UI.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/jasper-ui"
sed -e "s|@VERSION@|0.1.0|" "$macos_root/Info.plist.in" > "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
plutil -lint "$app/Contents/Info.plist"

codesign --force --sign "$identity" --timestamp=none --identifier org.jasper.ui.macos "$app"
codesign --verify --deep --strict --verbose=2 "$app"

echo "macos_app=$app"
echo "macos_profile=$profile"
echo "macos_universal=$universal"
lipo -info "$app/Contents/MacOS/jasper-ui"
shasum -a 256 "$app/Contents/MacOS/jasper-ui" "$app/Contents/Info.plist"
