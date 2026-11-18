#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

default_font=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
export JASPER_UI_TEST_FONT="${JASPER_UI_TEST_FONT:-$default_font}"
export JASPER_UI_TEST_LATIN_FONT="${JASPER_UI_TEST_LATIN_FONT:-$JASPER_UI_TEST_FONT}"
export JASPER_UI_TEST_CJK_FONT="${JASPER_UI_TEST_CJK_FONT:-/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc}"
export JASPER_UI_TEST_BOLD_FONT="${JASPER_UI_TEST_BOLD_FONT:-/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf}"
export JASPER_UI_TEST_ITALIC_FONT="${JASPER_UI_TEST_ITALIC_FONT:-/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf}"

require_font() {
    local variable="$1"
    local path="${!variable}"
    if [[ ! -f "$path" || ! -r "$path" ]]; then
        printf '%s must name a readable font file, got %q\n' "$variable" "$path" >&2
        exit 2
    fi
}

for variable in \
    JASPER_UI_TEST_FONT \
    JASPER_UI_TEST_LATIN_FONT \
    JASPER_UI_TEST_CJK_FONT \
    JASPER_UI_TEST_BOLD_FONT \
    JASPER_UI_TEST_ITALIC_FONT; do
    require_font "$variable"
done

if [[ "${1:-}" == "--check-env" ]]; then
    for variable in \
        JASPER_UI_TEST_FONT \
        JASPER_UI_TEST_LATIN_FONT \
        JASPER_UI_TEST_CJK_FONT \
        JASPER_UI_TEST_BOLD_FONT \
        JASPER_UI_TEST_ITALIC_FONT; do
        printf '%s=%s\n' "$variable" "${!variable}"
    done
    exit 0
fi

cargo="${CARGO:-}"
if [[ -z "$cargo" ]]; then
    if command -v cargo >/dev/null 2>&1; then
        cargo="$(command -v cargo)"
    elif [[ -x "$HOME/.cargo/bin/cargo" ]]; then
        cargo="$HOME/.cargo/bin/cargo"
    else
        printf 'cargo executable not found; set CARGO explicitly\n' >&2
        exit 2
    fi
fi
if ! command -v "$cargo" >/dev/null 2>&1; then
    printf 'cargo executable not found: %s\n' "$cargo" >&2
    exit 2
fi

# Avoid retaining 60+ GiB of debug/incremental objects during the large feature union.
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0

cd "$repo_root"
"$cargo" test --locked --all-features --tests -- --test-threads=1 "$@"
"$cargo" test --locked --all-features --doc
