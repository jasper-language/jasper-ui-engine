#!/bin/bash
# Diagnostic tee only: same solver, same problem; keeps the raw model across cbmc exit/timeout.
set -o pipefail
date -u +%FT%TZ > $HOME/work/cp02-bitwuzla/run3.solver-start
$HOME/tools/bitwuzla-0.8.2-install/bin/bitwuzla "$@" | tee $HOME/work/cp02-bitwuzla/run3.bitwuzla.out
rc=$?
echo "rc=$rc $(date -u +%FT%TZ)" > $HOME/work/cp02-bitwuzla/run3.solver-end
exit $rc
