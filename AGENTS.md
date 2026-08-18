# AGENTS.md — pdviewx

A semantic, GPU-native molecular rendering engine in Rust, delivered as a
library. It renders a structure — parsed by `pdbiox` — into an interactive scene
where the biology drives the picture. It does **not** parse, fetch, dock,
simulate, or open a window; those are out of scope or the caller's job.

`RULES.md` is the binding code style guide — read it before writing code. `docs/`
is the normative specification; read the **one** numbered file for the area you
are changing when you need the contract, and don't preload the rest.

## Stack and tooling

- **Rust edition 2024**, stable toolchain (pinned in `rust-toolchain.toml`). No
  nightly in the core.
- **wgpu + WGSL** is the default GPU backend; `glam` for math; `bytemuck` for GPU
  casts. An experimental Vulkan (`ash`) backend is feature-gated behind `vulkan`.
- **`pdbiox` is an external dependency** (`github.com/miguelcsx/pdbiox`) — not a
  submodule, not vendored. Don't reimplement parsing or the structural model; use
  `pdbiox`. If detection (interactions, SASA, secondary structure) is missing, add
  it there, not here.

## Commands

```bash
cargo build --workspace                      # build everything
cargo test  --workspace                      # all tests
cargo test  -p pdviewx-core                  # one crate
cargo run   --example pocket --release       # drive the engine in a window
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Tests for `foo.rs` live in the sibling `foo_tests.rs`, with sentence-form names.
The repo is pre-implementation: most crates are still stubs, so a command may have
little to compile or run yet — the command is still the right one.

## Crates

Dependencies point inward (a crate uses lower layers only). `#![forbid(unsafe_code)]`
everywhere except `pdviewx-gpu-vulkan` and `bytemuck` casts.

| Crate | Does |
|---|---|
| `pdviewx-math` | glam-based math: splines, parallel-transport frames, camera, AABB/BVH |
| `pdviewx-core` | The scene graph (columnar), `AtomGpu`/`BondGpu` packing, the zero-copy `pdbiox` coordinate seam |
| `pdviewx-gpu` | The backend-agnostic HAL (device/buffer/pass traits); no backend code |
| `pdviewx-gpu-wgpu` | Default wgpu backend |
| `pdviewx-gpu-vulkan` | β Vulkan/RTX backend (audited `unsafe`) |
| `pdviewx-shaders` | Shared WGSL (impostor intersection, lighting); SPIR-V via naga |
| `pdviewx-geometry` | GPU geometry: impostor packing, ribbons, surfaces, BVH build |
| `pdviewx-render` | Render graph; realtime + quality modes |
| `pdviewx-semantic` | The semantic layer: focus+context, materials, LOD, interactions |
| `pdviewx` | Facade: feature-gated re-exports, no logic |
| `pdviewx-bench` | Benchmark harness |

Callers import from `pdviewx` only. Backends are chosen by capability, never named
in the public API.

## Project-specific gotchas

Things an agent would get wrong without being told (full rules in `RULES.md`):

- **Coordinates are borrowed, never copied.** Don't copy `pdbiox`'s coordinate
  column into the scene — upload it directly (the zero-copy seam). An atom is 32
  bytes of instance data plus a borrowed position.
- **Draw the GPU way.** Don't issue a draw call per atom/bond/residue — impostors
  are instanced and large scenes use indirect draw whose counts a compute pass
  writes.
- **No meshes for atoms/bonds.** Don't tessellate a sphere or cylinder — draw an
  analytic impostor quad and intersect it in the fragment shader.
- **One WGSL source.** Don't fork a shader per backend — author it once in
  `pdviewx-shaders`; the Vulkan backend compiles the same WGSL to SPIR-V.
- **Errors are values.** Don't `unwrap`/`expect` outside tests, and don't panic on
  device loss or surface loss — return the typed error and let the caller recover.
- **`unsafe` lives in two places only** — `pdviewx-gpu-vulkan` and `bytemuck`
  casts. Anywhere else is a bug.

## Verification

Green means all of these pass (also run by `scripts/verify.sh` once it exists):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
grep -rn "unwrap" crates/ | grep -v "_tests.rs"          # must be empty
find crates -name "*.rs" | xargs wc -l | awk '$1>500'    # must be empty (file cap)
grep -rn "unsafe" crates/ | grep -v "pdviewx-gpu-vulkan" | grep -v "bytemuck"  # audit surface
```

## Commit scopes

One of: `math core gpu wgpu vulkan shaders geometry render semantic facade bench
examples spec repo ci`.
