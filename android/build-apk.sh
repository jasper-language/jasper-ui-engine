#!/usr/bin/env bash
# Build the Android reference host in the project build VM.
# 在项目构建 VM 内构建 Android 参考宿主。
set -euo pipefail

android_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$android_root/.." && pwd)
ndk=${ANDROID_NDK_HOME:-/usr/lib/android-sdk/ndk/27.2.12479018}
sdk=${ANDROID_HOME:-/usr/lib/android-sdk}
target=aarch64-linux-android
api=24
toolchain="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"

test -d "$ndk"
test -x "$toolchain/aarch64-linux-android${api}-clang"
command -v aapt2 >/dev/null
command -v zipalign >/dev/null
command -v apksigner >/dev/null
command -v keytool >/dev/null

export CC_aarch64_linux_android="$toolchain/aarch64-linux-android${api}-clang"
export AR_aarch64_linux_android="$toolchain/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_INCREMENTAL=0

cd "$android_root"
cargo build --locked --release --target "$target"

out="$android_root/target/apk"
mkdir -p "$out/lib/arm64-v8a"
cp "$android_root/target/$target/release/libmain.so" "$out/lib/arm64-v8a/libmain.so"

aapt2 link \
    -o "$out/jasper-ui-unsigned.apk" \
    -I "$sdk/platforms/android-35/android.jar" \
    --manifest "$android_root/AndroidManifest.xml" \
    --min-sdk-version 24 \
    --target-sdk-version 35 \
    --version-code 1 \
    --version-name 0.1.0

(
    cd "$out"
    zip -0 -q jasper-ui-unsigned.apk lib/arm64-v8a/libmain.so
)

zipalign -f 4 "$out/jasper-ui-unsigned.apk" "$out/jasper-ui-aligned.apk"

keystore="$out/debug.keystore"
if [[ ! -f "$keystore" ]]; then
    keytool -genkeypair \
        -keystore "$keystore" \
        -storepass android \
        -alias androiddebugkey \
        -keypass android \
        -keyalg RSA \
        -keysize 2048 \
        -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi

apksigner sign \
    --ks "$keystore" \
    --ks-pass pass:android \
    --key-pass pass:android \
    --out "$out/jasper-ui.apk" \
    "$out/jasper-ui-aligned.apk"

apksigner verify --verbose "$out/jasper-ui.apk"
echo "android_apk=$out/jasper-ui.apk"
