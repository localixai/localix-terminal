#!/bin/sh
# Where the time goes: one engine fed one workload over and over (the bench's child binary, the
# full-size workload file `run.sh` wrote), watched by macOS's own sampler for four seconds, and
# the functions it was found in most, by their own time.
#
#   ./profile.sh ascii                 # llxterm on ascii
#   ./profile.sh tui rio-vt            # another engine (rio-vt runs in engine-apart)
set -eu
cd "$(dirname "$0")"
workload=${1:-ascii}
engine=${2:-llxterm}
file=target/workloads/$workload.bin
[ -f "$file" ] || { echo "no $file: run ./run.sh once first" >&2; exit 1; }
binary=target/release/engine
[ "$engine" = rio-vt ] && binary=target/release/engine-apart
"$binary" "$engine" time "$file" 400 0 > /dev/null &
pid=$!
sleep 1
out=$(mktemp)
sample "$pid" 4 -mayDie -file "$out" > /dev/null 2>&1 || true
kill "$pid" 2> /dev/null || true
wait "$pid" 2> /dev/null || true
# The sampler's own summary: each function by the samples where it was on top of the stack.
awk '/^Sort by top of stack/ { on = 1; next } on && NF { print } /^Binary Images/ { exit }' "$out" \
    | grep -v '^Binary Images' | head -30
rm -f "$out"
