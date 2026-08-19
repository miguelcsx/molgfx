#!/usr/bin/env bash
# Reference-adapter p99 gate for the two high-cardinality realtime fixtures.
set -uo pipefail
cd "$(dirname "$0")/.."

budget_ns=8333333
fail=0

run_gate() {
  local label="$1"
  shift
  local output
  output=$("$@") || return 1
  echo "$output"
  local p99
  p99=$(echo "$output" | sed -n 's/.*p99 \([0-9][0-9]*\) ns.*/\1/p' | tail -n 1)
  if [ -z "$p99" ] || [ "$p99" -gt "$budget_ns" ]; then
    echo "$label: FAIL (p99=${p99:-unavailable} ns, budget=$budget_ns ns)"
    return 1
  fi
  echo "$label: OK (p99=$p99 ns)"
}

run_gate "one-million points" \
  cargo run --release --example million_particles -- \
  1000000 target/visual-checks/million-points-120fps.png 300 memory points || fail=1

run_gate "400-pose docking swarm" \
  cargo run --release --example docking_swarm --features semantic -- \
  benchmarks/scenes/3PTB.cif BEN 400 \
  target/visual-checks/docking-400-120fps.png 300 || fail=1

if [ "$fail" -ne 0 ]; then
  exit 1
fi
