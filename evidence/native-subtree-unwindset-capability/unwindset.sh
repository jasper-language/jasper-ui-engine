#!/bin/bash
# Emits the CBMC --unwindset spec from the loop ids a previous Kani log printed.
# Bounds are claims, not assumptions: unwinding assertions stay on, so a low bound fails the run.
log=$1
grep -E "nwinding loop" "$log" | sed -E 's/^(Not u|U)nwinding loop //; s/ .*//' | sort -u | while read -r id; do
  case "$id" in
    *correct_childrens_parent*) n=1;;   # two entries never create an internal node
    *search_tree*) n=2;;                # a leaf root ends the descent in the first pass
    *insert_recursing*) n=2;;           # no split below capacity
    *) n=4;;                            # find_key_index, production loop and oracle: at most three passes
  esac
  printf '%s:%s,' "$id" "$n"
done | sed 's/,$//'
