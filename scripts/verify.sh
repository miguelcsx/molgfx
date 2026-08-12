#!/usr/bin/env bash
# The single verification gate. Green means every check below passed.
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
step() { echo "==> $1"; }

step "cargo fmt --all --check"
cargo fmt --all --check || fail=1

step "cargo clippy --workspace --all-targets (zero warnings)"
cargo clippy --workspace --all-targets -- -D warnings || fail=1

step "cargo test --workspace"
cargo test --workspace || fail=1

step "doc checks"
./scripts/check-docs.sh || fail=1

step "no unwrap outside test files"
if grep -rn "unwrap" crates/ --include="*.rs" | grep -v "_tests.rs" | grep -v "^Binary"; then
  echo "unwrap found outside test files"; fail=1
fi

step "no source file over 500 lines"
if find crates -name "*.rs" -print0 | xargs -0 wc -l | awk '$1>500 && $2 != "total" {print; found=1} END {exit found}'; then :; else
  echo "file cap exceeded"; fail=1
fi

step "unsafe audit surface (vulkan backend and bytemuck only)"
if grep -rn "unsafe" crates/ --include="*.rs" | grep -v "pdviewx-gpu-vulkan" | grep -v "bytemuck" | grep -v "forbid(unsafe_code)"; then
  echo "unsafe outside the audited boundaries"; fail=1
fi

if [ "$fail" -ne 0 ]; then echo "VERIFY: FAIL"; exit 1; fi
echo "VERIFY: OK"
