# AGENTS.md — molgfx

A semantic, GPU-native molecular rendering engine in Rust, delivered as a
library. It renders a structure — parsed by `molframe` — into an interactive scene
where the biology drives the picture. It does **not** parse, fetch, dock,
simulate, or open a window; those are out of scope or the caller's job.

`RULES.md` is the binding code style guide — read it before writing code.

## Stack and tooling

- **Rust edition 2024**, stable toolchain (pinned in `rust-toolchain.toml`). No
  nightly.
- **wgpu + WGSL** is the GPU backend; `glam` for math; `bytemuck` for GPU casts.
  There is exactly one backend, `molgfx-wgpu`, and callers never name it.
- **`molframe` is an external dependency** (`github.com/miguelcsx/molframe`) — not a
  submodule, not vendored. Don't reimplement parsing or the structural model; use
  `molframe`. If detection (interactions, SASA, secondary structure) is missing, add
  it there, not here.

## Commands

```bash
cargo build --workspace                      # build everything
cargo test  --workspace                      # all tests
cargo test  -p molgfx-core                  # one crate
cargo run   -p molgfx-bench --bin focus_profile --release   # drive the engine
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Tests for `foo.rs` live in the sibling `foo_tests.rs`, with sentence-form names.
A module directory may instead carry one `tests.rs` for the modules beside it.
Most crates are still stubs, so a command may have little to compile or run yet —
the command is still the right one.

## Crates

Dependencies point inward (a crate uses lower layers only). `#![forbid(unsafe_code)]`
everywhere; the only `unsafe` is `bytemuck` POD casts for GPU upload.

| Crate | Does |
|---|---|
| `molgfx-math` | glam-based math: splines, parallel-transport frames, camera, AABB/BVH |
| `molgfx-core` | The scene graph (columnar), `AtomGpu`/`BondGpu` packing, the zero-copy `molframe` coordinate seam |
| `molgfx-gpu` | The backend-agnostic HAL (device/buffer/pass traits); no backend code |
| `molgfx-wgpu` | The wgpu implementation of the HAL; chosen by capability, never named by callers |
| `molgfx-shaders` | Shared WGSL, composed and validated at build time (naga) |
| `molgfx-geometry` | GPU geometry: impostor packing, ribbons, surfaces, BVH build |
| `molgfx-render` | Render graph; realtime + quality modes |
| `molgfx-semantic` | The semantic layer: focus+context, materials, LOD, interactions |
| `molgfx` | Facade: one module per inner crate, feature-gated, no logic |
| `molgfx-py` | PyO3 type adapters and registration over the facade |
| `molgfx-wasm` | Browser bindings over the same declarative scene and engine |
| `molgfx-bench` | Benchmark harness |

Callers import from `molgfx` only. Backends are chosen by capability, never named
in the public API. The facade publishes each inner crate as a module of the same
name — `molgfx::core`, `molgfx::render` — so a name has one home and no collision
with a same-named concept in another layer; `molgfx::prelude` is the curated
shortcut for ordinary work.

## Project-specific gotchas

Things an agent would get wrong without being told (full rules in `RULES.md`):

- **Coordinates are borrowed, never copied.** Don't copy `molframe`'s coordinate
  column into the scene — upload it directly (the zero-copy seam). An atom is a
  20-byte instance record with no position field; the GPU gathers the position
  by `entity_id` from the borrowed `coords[]` column.
- **Draw the GPU way.** Don't issue a draw call per atom/bond/residue — impostors
  are instanced and large scenes use indirect draw whose counts a compute pass
  writes.
- **No meshes for atoms/bonds.** Don't tessellate a sphere or cylinder — draw an
  analytic impostor quad and intersect it in the fragment shader.
- **One WGSL source.** Don't fork a shader — author it once in `molgfx-shaders`;
  there is no second shader dialect to keep in sync.
- **Errors are values.** Don't `unwrap`/`expect` outside tests, and don't panic on
  device loss or surface loss — return the typed error and let the caller recover.
- **`unsafe` lives in one place only** — `bytemuck` casts. Anywhere else is a bug.
- **No generated files and no local scripts.** A binding surface is either written
  by hand or produced by a committed generator; there is no codegen step to re-run.

## Verification

Green means all of these pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
grep -rn "unwrap" crates/ --include="*.rs" | grep -v "_tests.rs" | grep -v "/tests.rs" | grep -v "generic_tests/"   # must be empty
find crates \( -name "*.rs" -o -name "*.wgsl" \) -print0 | xargs -0 wc -l | awk '$1>500 && $2 != "total" {print}'   # must be empty (file cap)
grep -rnE "(^|[^A-Za-z_])unsafe([[:space:]]*\{|[[:space:]]+(fn|impl|trait|extern|static|mut))" crates/ --include="*.rs" | grep -v bytemuck   # must be empty
```

No check reads a scene corpus, so a checkout with no `benchmarks/` passes the whole
list: the benchmarks resolve data through `crates/molgfx-bench/src/fixtures.rs` and
skip when none is present.

## Commit scopes

One of: `math core gpu wgpu shaders geometry render semantic facade py wasm bench
spec repo ci`.
