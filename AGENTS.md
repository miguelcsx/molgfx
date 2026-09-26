# AGENTS.md — molgfx

A semantic, GPU-native molecular rendering engine in Rust, delivered as a
library. It renders a structure — parsed by `molframe` — into an interactive scene
where the biology drives the picture. It does **not** parse, fetch, dock,
simulate, or open a window; those are out of scope or the caller's job.

`RULES.md` is the binding code style guide — read it before writing code.

## Stack and tooling

- **Nix** is the build system and package manager, use it over `cargo`, use `nix develop` to set up the development environment.
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
cargo build                                  # engine only (fast inner loop; see default-members)
cargo build --workspace                      # everything, including py/wasm/bench
cargo test  --workspace                      # all tests
cargo test  -p molgfx-core                  # one crate
cargo check -p molgfx-wasm --target wasm32-unknown-unknown   # the wasm leaf, on its real target
cargo bench -p molgfx-bench                    # declarative/runtime performance matrix
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
```

The workspace's `default-members` covers the engine crates only. Python, WASM,
and benchmark leaves remain outside the no-selector inner loop, while every
`--workspace` command covers them. `molframe-py` is domain-featured; do not
restore its former unconditional `full` dependency.

Tests for `foo.rs` live in the sibling `foo_tests.rs`, with sentence-form names.
A module directory may instead carry one `tests.rs` for the modules beside it.
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
| `molgfx-api` | Declarative values, semantic IDs, patches, resolved-scene lowering |
| `molgfx-command` | The authoring command language: typed commands, parser, session, undo/redo |
| `molgfx` | Curated facade: `Scene`, `Renderer`, specs, values, and explicit namespaces |
| `molgfx-py` | PyO3 type adapters and registration over the facade |
| `molgfx-wasm` | Browser bindings over the same declarative scene and engine |
| `molgfx-bench` | Benchmark harness |

Ordinary callers import from `molgfx` only. Backends are chosen by capability,
never named in the public API. The facade deliberately does not re-export inner
crates or provide a `Deref` escape hatch. Low-level schema records live under
`molgfx::schema`; advanced mechanisms live in explicit curated namespaces.
`molgfx-py` and `molgfx-wasm` depend on `molgfx` alone — a binding that needs
something inward is a missing curated namespace, not a reason to add a
dependency.

`molgfx-api` is organized one directory per domain concept — `scene/`, `patch/`,
`visual/`, `spec/`, `representation/`, `science/`, `render/`, `interop/` — with
tests beside the code they cover. Do not add a flat module at its root, and do
not use `#[path]` or `include!` to keep one module's text in several files: a
file that has outgrown the cap has outgrown its responsibility, so split the
module.

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
- **No hand-written versions.** Nothing in the code defines a schema, wire or
  format version, and no identifier or literal carries a `v1`/`v2` suffix.
  Package versions live in `Cargo.toml`. Incompatible serialized input fails on
  its own shape. (`MolViewSpec v1` is an external standard MolGFX reads, not a
  version MolGFX defines.)
- **One definition of a visual input.** Everything a visual program may read —
  renderer scalars and vectors, interaction channels, bound atom columns — is
  resolved through `molgfx-api`'s input registry. A style that authors cleanly
  must lower cleanly; two validators that disagree is the bug that registry
  exists to prevent.
- **No `unsafe` in product code.** POD upload types use audited `bytemuck` derives.
- **No ad-hoc generated source or local policy scripts.** Build-time WGSL
  composition is part of the committed build architecture.

## Verification

Green means all of these pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check  -p molgfx-wasm --target wasm32-unknown-unknown
cargo clippy -p molgfx-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings
python -m unittest discover -s python/tests
python -m mypy.stubtest molgfx._engine
grep -rnE '#\[(allow|expect)\b' crates/ --include='*.rs'  # must be empty
grep -rn "unwrap" crates/ --include="*.rs" | grep -v "_tests.rs" | grep -v "/tests.rs" | grep -v "generic_tests/"   # must be empty
find crates \( -name "*.rs" -o -name "*.wgsl" \) -print0 | xargs -0 wc -l | awk '$1>500 && $2 != "total" {print}'   # must be empty (file cap)
grep -rnE "(^|[^A-Za-z_])unsafe([[:space:]]*\{|[[:space:]]+(fn|impl|trait|extern|static|mut))" crates/ --include="*.rs" | grep -v bytemuck   # must be empty
```

`--workspace --all-features` type-checks and lints `molgfx-wasm` too, but only
as an `rlib` for the host target — every item in that crate is
`#[cfg(target_arch = "wasm32")]`-gated, so a host build cfg-strips all of it
away first. The two `wasm32-unknown-unknown` lines above are the only ones
that actually verify that crate's code; they aren't redundant with the
`--workspace` commands above them.

Repository-policy greps belong in CI workflow steps, never compiled product
tests. No check reads a scene corpus, so a checkout without `benchmarks/` passes;
benchmark fixtures resolve data only through the harness.

## Commit scopes

One of: `math core gpu wgpu shaders geometry render semantic api command facade py
wasm bench spec repo ci`.
