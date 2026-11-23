#!/bin/sh
# Every benchmark, in one run: the engines built (rio-vt's apart, in apart/), throughput, then
# memory, each writing its results/; then results/latest.md (latest.sh), and the run held to
# budgets.json. Nothing else should run meanwhile: the timings are the machine's whole attention.
# `--quick` (or BENCH_SMALL=1) is the smoke run instead: small inputs, one round, results under
# target/smoke/, to prove each core works.
set -eu
cd "$(dirname "$0")"
quick=""
case "${1:-}" in --quick) quick="--quick" ;; esac
[ "${BENCH_SMALL:-}" = 1 ] && quick="--quick"
cargo build --quiet --release --bins
(cd apart && cargo build --quiet --release)
for bench in throughput memory; do
    # shellcheck disable=SC2086 # $quick is one flag or none
    cargo run --quiet --release --bin "$bench" -- $quick > /dev/null
done
./latest.sh $quick
# shellcheck disable=SC2086
cargo run --quiet --release --bin check -- $quick
