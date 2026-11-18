#!/usr/bin/env bash
# Compile the host-independent no_std + alloc core for a bare-metal target.
set -euo pipefail

toolchain=${JASPER_NO_STD_TOOLCHAIN:-nightly-2026-10-17}
target=${JASPER_NO_STD_TARGET:-x86_64-unknown-none}
cargo_home=${CARGO_HOME:-"$HOME/.cargo"}
cargo_bin=${CARGO:-"$cargo_home/bin/cargo"}
rustup_bin=${RUSTUP:-"$cargo_home/bin/rustup"}

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if [[ ! -x "$cargo_bin" ]]; then
    printf 'cargo is not executable: %s\n' "$cargo_bin" >&2
    exit 2
fi
if [[ ! -x "$rustup_bin" ]]; then
    printf 'rustup is not executable: %s\n' "$rustup_bin" >&2
    exit 2
fi

if ! RUSTUP_TOOLCHAIN=$toolchain "$rustup_bin" target list --installed | grep -qx "$target"; then
    printf 'missing Rust target %s for toolchain %s\n' "$target" "$toolchain" >&2
    printf 'install it with: rustup +%s target add %s\n' "$toolchain" "$target" >&2
    exit 2
fi

runtime_version=$(RUSTUP_TOOLCHAIN=$toolchain "$rustup_bin" run "$toolchain" rustc --version)
printf 'toolchain=%s\n' "$runtime_version"
printf 'target=%s\n' "$target"
printf 'feature=no-std-core\n'
sha256sum Cargo.toml Cargo.lock

RUSTUP_TOOLCHAIN=$toolchain "$cargo_bin" check --locked \
    --target "$target" \
    --no-default-features --features no-std-core --lib

printf 'no_std_core=pass\n'
