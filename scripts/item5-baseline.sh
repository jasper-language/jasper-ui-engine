#!/usr/bin/env bash
# Repeat the item 5 reference CPU scene and retain raw output for same-VM comparisons.
set -euo pipefail
if (( $# != 2 )); then
    echo "usage: $0 FONT OUTPUT_DIR" >&2
    exit 2
fi
ui_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
font=$1
out=$2
mkdir -p "$out"
cd "$ui_root"
JASPER_UI_TEST_FONT=$font cargo run --locked --release \
    --features native-engine-cpu --example full_scene -- \
    "$font" "$out/full_scene.ppm" | tee "$out/full_scene.log"
