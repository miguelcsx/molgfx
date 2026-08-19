#!/usr/bin/env bash
# The single verification gate. Green means every check below passed.
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
step() { echo "==> $1"; }

step "cargo fmt --all --check"
cargo fmt --all --check || fail=1

step "cargo clippy --workspace --all-targets --all-features (zero warnings)"
cargo clippy --workspace --all-targets --all-features -- -D warnings || fail=1

step "cargo test --workspace --all-features"
cargo test --workspace --all-features || fail=1

step "browser WebGPU facade and JavaScript binding cross-build"
cargo check -p pdviewx -p pdviewx-wasm --target wasm32-unknown-unknown --lib --all-features || fail=1

step "fresh Python wheel facade and stub audit"
maturin build --release --out target/wheels || fail=1
wheel=$(find target/wheels -maxdepth 1 -name 'pdviewx-*.whl' -print | head -n 1)
if [ -n "$wheel" ]; then
  PYTHONDONTWRITEBYTECODE=1 python3 scripts/python-binding-audit.py "$wheel" || fail=1
else
  echo "fresh pdviewx wheel not found"; fail=1
fi

step "golden-image corpus (binding on the reference adapter, advisory elsewhere)"
cargo run --release --example golden -p pdviewx --features semantic || fail=1

step "doc checks"
./scripts/check-docs.sh || fail=1

step "public facade capability audit"
PYTHONDONTWRITEBYTECODE=1 python3 scripts/graphics_engine_facade_audit.py --repo . --check || fail=1

step "no legacy object-at-a-time creation facade"
if rg -n 'pub fn (add_ellipsoid|add_carbohydrate_symbol|add_filled_planar_region|add_particle|add_particles|reserve_primitives|represent_volume|represent_isosurface|represent_medium|represent_volume_slice|represent_liquid_surface|represent_volume_region|represent_segmented_volume)\b' crates/ --glob '*.rs'; then
  echo "legacy creation facade found"; fail=1
fi

step "graphics-engine gap ledger audit"
PYTHONDONTWRITEBYTECODE=1 python3 scripts/graphics_engine_ledger_audit.py --check || fail=1

step "VMD feature manifest audit"
PYTHONDONTWRITEBYTECODE=1 python3 scripts/graphics_engine_vmd_probe.py --check --summary || fail=1

step "no unwrap outside test files"
if grep -rn "unwrap" crates/ --include="*.rs" | grep -v "_tests.rs" | grep -v "^Binary"; then
  echo "unwrap found outside test files"; fail=1
fi

step "no source file over 500 lines (Rust and WGSL)"
if find crates \( -name "*.rs" -o -name "*.wgsl" \) -print0 | xargs -0 wc -l | awk '$1>500 && $2 != "total" {print; found=1} END {exit found}'; then :; else
  echo "file cap exceeded"; fail=1
fi

step "unsafe audit surface (bytemuck casts only)"
if grep -rn "unsafe" crates/ --include="*.rs" | grep -v "bytemuck" | grep -v "forbid(unsafe_code)"; then
  echo "unsafe outside the audited boundaries"; fail=1
fi

if [ "$fail" -ne 0 ]; then echo "VERIFY: FAIL"; exit 1; fi
echo "VERIFY: OK"
