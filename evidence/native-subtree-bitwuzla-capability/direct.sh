#!/bin/bash
W=$HOME/work/cp02-bitwuzla; cd $W
export PATH=$HOME/.kani/kani-0.68.0/bin:$HOME/tools/bitwuzla-0.8.2-install/bin:$PATH TMPDIR=$W/tmp
ulimit -v 16000000
date -u +%FT%TZ > direct.log.started
read -r -a ARGV < cbmc.argv
timeout 3500 nice -n 5 "${ARGV[@]}" --stop-on-fail --trace > direct.log 2>&1
echo "rc=$? $(date -u +%FT%TZ)" > direct.log.done
