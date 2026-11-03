#!/bin/bash
# $1=log $2=extra label; args fixed: original domain, real map, real drop, unwinding assertions on.
W=$HOME/work/cp02-bitwuzla; cd $W
export PATH=$HOME/.cargo/bin:$HOME/tools/bitwuzla-0.8.2-install/bin:$PATH TMPDIR=$W/tmp
ulimit -v 16000000
date -u +%FT%TZ > "$1.started"
timeout 3500 nice -n 5 cargo kani --manifest-path verification/native-activation/Cargo.toml --harness "$2" --output-format regular --solver bitwuzla -Z unstable-options --cbmc-args --stop-on-fail --trace --object-bits 13 --unwind 4 --unwindset "$(cat unwindset.txt)" > "$1" 2>&1
echo "rc=$? $(date -u +%FT%TZ)" > "$1.done"
