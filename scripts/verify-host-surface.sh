#!/usr/bin/env bash
# Verify the production Surface lifecycle in the restricted build VM.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
manifest="$root/verification/host-surface/Cargo.toml"
out="$root/evidence/native-host-render-lifecycle"
mkdir -p "$out/fuzz-artifacts"
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR="$root/experiments/blitz-host/target"
export JASPER_UI_TEST_FONT="${JASPER_UI_TEST_FONT:-/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf}"

if [[ ! -f "$root/verification/host-surface/Cargo.lock" ]]; then
    cargo generate-lockfile --manifest-path "$manifest"
fi
cargo test --manifest-path "$manifest" --locked 2>&1 | tee "$out/proptest.log"
MIRIFLAGS=-Zmiri-strict-provenance cargo +nightly miri test --manifest-path "$manifest" --locked \
    -- --test-threads=1 --skip successful_cycles_stay_active_and_monotone \
    2>&1 | tee "$out/miri.log"
cargo kani --manifest-path "$manifest" 2>&1 | tee "$out/kani.log"
cp "$root/verification/host-surface/Cargo.lock" "$out/Cargo.lock"
cargo generate-lockfile --manifest-path "$root/fuzz/Cargo.toml"
cp "$root/fuzz/Cargo.lock" "$out/fuzz-Cargo.lock"
cargo +nightly fuzz run host_surface "$root/fuzz/seeds/host_surface" -- \
    -max_total_time=10 -max_len=512 -rss_limit_mb=1024 -timeout=5 \
    -artifact_prefix="$out/fuzz-artifacts/" 2>&1 | tee "$out/fuzz.log"
cargo test --locked --features native-blitz-cpu-pending --test native_host_renderer \
    2>&1 | tee "$out/cpu-integration.log"
cargo test --locked --features gpu-pending --test native_gpu_host_view \
    2>&1 | tee "$out/gpu-integration.log"
