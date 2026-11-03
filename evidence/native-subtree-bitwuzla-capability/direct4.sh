#!/bin/bash
W=$HOME/work/cp02-bitwuzla; cd $W
export PATH=$W/shim4:$HOME/.kani/kani-0.68.0/bin:$HOME/tools/bitwuzla-0.8.2-install/bin:$PATH TMPDIR=$W/tmp
ulimit -v 16000000
date -u +%FT%TZ > run4.started
read -r -a ARGV < cbmc.argv
P=(); while read -r n; do P+=(--property "$n"); done < real.props
timeout -k 30 6300 nice -n 5 "${ARGV[@]}" "${P[@]}" --stop-on-fail --trace > run4.log 2>&1
echo "rc=$? $(date -u +%FT%TZ)" > run4.done
