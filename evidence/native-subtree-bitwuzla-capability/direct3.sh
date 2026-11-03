#!/bin/bash
W=$HOME/work/cp02-bitwuzla; cd $W
export PATH=$W/shim:$HOME/.kani/kani-0.68.0/bin:$HOME/tools/bitwuzla-0.8.2-install/bin:$PATH TMPDIR=$W/tmp
ulimit -v 16000000
date -u +%FT%TZ > run3.started
read -r -a ARGV < cbmc.argv
timeout -k 30 9000 nice -n 5 "${ARGV[@]}" --stop-on-fail --trace > run3.log 2>&1
echo "rc=$? $(date -u +%FT%TZ)" > run3.done
