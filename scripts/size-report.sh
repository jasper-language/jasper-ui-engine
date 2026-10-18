#!/usr/bin/env bash
# Build release artifacts and print their sizes.
# 默认只用 2 个核（0-1）；可用 JASPER_CORES / CARGO_BUILD_JOBS 覆盖。
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

TASKSET=""
if [ "$(uname -s)" = "Linux" ] && command -v taskset >/dev/null 2>&1; then
  TASKSET="taskset -c ${JASPER_CORES:-0-1}"
fi

echo "== build (jobs=$CARGO_BUILD_JOBS, cores=${JASPER_CORES:-0-1}) =="
$TASKSET cargo build --release --lib --example hello

LIB="target/release/libjasper_ui_engine.rlib"
BIN="target/release/examples/hello"
[ -f "$LIB" ] || { echo "missing $LIB" >&2; exit 1; }
[ -f "$BIN" ] || { echo "missing $BIN" >&2; exit 1; }

size_of() { wc -c < "$1" | tr -d ' '; }
mib() { awk -v b="$1" 'BEGIN { printf "%.2f", b / 1048576 }'; }

LIB_BYTES="$(size_of "$LIB")"
BIN_BYTES="$(size_of "$BIN")"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
STRIPPED="n/a"
STRIPPED_BYTES="n/a"
if command -v strip >/dev/null 2>&1; then
  cp "$BIN" "$TMP/hello"
  strip --strip-all "$TMP/hello" 2>/dev/null || strip "$TMP/hello" 2>/dev/null || true
  STRIPPED="$TMP/hello"
  STRIPPED_BYTES="$(size_of "$STRIPPED")"
fi

echo
printf '%-42s %12s %10s\n' "artifact" "bytes" "MiB"
printf '%-42s %12s %10s\n' "release rlib" "$LIB_BYTES" "$(mib "$LIB_BYTES")"
printf '%-42s %12s %10s\n' "release example (not stripped)" "$BIN_BYTES" "$(mib "$BIN_BYTES")"
if [ "$STRIPPED_BYTES" != "n/a" ]; then
  printf '%-42s %12s %10s\n' "release example (stripped)" "$STRIPPED_BYTES" "$(mib "$STRIPPED_BYTES")"
fi
echo
echo "notes:"
echo "- rlib is the Rust library archive, not a final app."
echo "- example includes the Rust runtime + taffy; no window/GPU/font stack yet."
echo "- strip size is a rough lower bound; link-time/LTO/platform differ."
