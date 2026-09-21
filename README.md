# MolGFX

MolGFX is a semantic, GPU-native molecular rendering library for Rust, Python,
and WebGPU. It turns a MolFrame structure plus an immutable declarative scene
description into renderer-owned physical resources. MolGFX does not parse,
fetch, dock, simulate, or open an application window.

## Architecture

The system has three boundaries:

1. **Authoring contract** — immutable selections, representation and color
   specifications, visual-expression DAGs, `SceneSpec`, and `ScenePatch`.
2. **Resolved scene** — a mutable `Scene` retains MolFrame storage, assigns
   monotonic semantic IDs, evaluates queries, tracks fine-grained revisions,
   and applies patches atomically.
3. **Physical renderer** — `Renderer` owns the device, shared buffers, derived
   caches, render graph, upload staging, and readback resources. Backend types
   do not appear in the facade.

The serialized `SceneSpec` contains data-source descriptors and semantic state,
not coordinate arrays or GPU records. This keeps local scenes zero-copy while
allowing the same contract to travel to Python and WASM.

## Rust

```rust,no_run
use molgfx::{Renderer, Scene, rep, sel};

# fn run(structure: &molframe::Structure) -> Result<(), molgfx::Error> {
let mut scene = Scene::from_structure(structure)?;
scene.add(rep::cartoon(sel::protein()))?;
scene.add(rep::ball_and_stick(sel::ligands()))?;
scene.focus(sel::ligands())?;

let mut renderer = Renderer::new()?;
let image = renderer.render_image(&scene, (1920, 1080))?;
image.save("structure.png")?;
# Ok(())
# }
```

`molgfx::sel` is MolFrame's query builder, re-exported directly; MolGFX has no
second molecular query language. `Scene::from_structure` clones MolFrame's
shared storage handle and uploads its borrowed `[[f32; 3]]` coordinate column
directly. Atom instance records therefore contain identity and appearance, not
duplicated positions.

Representations are typed values constructed in `molgfx::rep`: `cartoon`,
`ball_and_stick`, `spacefill`, `licorice`, `lines`, `points`, `surface`,
`nucleic_acid`, `bases`, `base_pairs`, and `glycan`. Cartoon recipes such as
rocket styling remain recipes rather than new representation kinds.

Only `Scene` and `Renderer` are mutable. A transaction validates its complete
result and commits one revision:

```rust,no_run
# use molgfx::{Scene, rep, sel};
# fn edit(scene: &mut Scene) -> Result<(), molgfx::Error> {
let ligand = scene.add(rep::licorice(sel::ligands()))?;
let patch = scene.transaction(|tx| {
    tx.set_opacity(ligand, 0.65);
    tx.set_visible(ligand, true);
    Ok(())
})?;
let json = patch.to_json()?;
# let _ = json;
# Ok(())
# }
```

Patches carry a base revision and typed operations. Conflicts and invalid final
states leave the scene unchanged. `ScenePatch::inverse` creates an undo patch
against the exact base specification.

## Python

Python uses the same contract and MolFrame selection objects:

```python
import molframe
import molgfx

structure = molframe.read("structure.cif")
scene = molgfx.Scene(structure)
scene.add(molgfx.rep.cartoon(target=molgfx.sel.protein()))
scene.add(molgfx.rep.ball_and_stick(target=molgfx.sel.ligands()))
scene.focus(molgfx.sel.ligands())

renderer = molgfx.Renderer()
renderer.render_image(scene, size=(1920, 1080)).save("structure.png")
```

Arguments are keyword-only where builders have multiple policies. Python
retains the MolFrame capsule instead of converting coordinates through NumPy.
The public exception hierarchy is `MolgfxError`, `SpecError`, and
`RevisionConflict`; the shipped stubs contain no `Any` escape hatch.

Notebook interaction uses `molgfx.viewer.Viewer`. The AnyWidget sends the
canonical scene JSON, compact BinaryCIF source buffers, and subsequent patches
to the packaged `molgfx-wasm` module. That module parses the same contract and
renders directly into a browser canvas with WebGPU. Resize remains local to the
browser and no rendered frame pixels travel back through the Python kernel.

## Wire contract and interoperability

`SceneSpec` and `ScenePatch` are deterministic JSON values consumed by Rust,
Python, and WASM. Runtime data bindings remain out of band and are identified by
URI and content hash. `molgfx::interop` imports and exports the compatible
MolViewSpec v1 subset as `.mvsj` or `.mvsx`; unsupported nodes produce explicit
diagnostics, and MolGFX metadata is preserved in a namespaced extension.

`molgfx::streaming::DataSource` is the bounded asynchronous provider contract.
Requests are prioritized and batched, cancellation is cooperative, failures
propagate, and shutdown is explicit. Residency tickets, page handles, and upload
details are intentionally renderer internals.

## Performance invariants

- Coordinates are borrowed from MolFrame and never copied into scene records.
- Atoms and bonds use analytic instanced impostors, not tessellated meshes.
- Culling writes indirect draw counts on the GPU.
- Upload staging uses bounded reusable arenas; streaming work is batched.
- Picking uses one aligned readback buffer and one mapping operation.
- BVH, surface, and render-graph resources are derived caches with explicit
  revision keys and memory accounting.
- WGSL is composed and validated once from `molgfx-shaders`; there is no SPIR-V
  output or second maintained shader dialect.

Use `explain()` and stable hashes to inspect authoring values and renderer plans
without exposing physical handles.

## Crates and validation

Ordinary callers depend on `molgfx`. Inner crates are separately versioned for
expert integrations, but are not re-exported through the facade.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p molgfx-wasm --target wasm32-unknown-unknown
cargo clippy -p molgfx-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings
python -m unittest discover -s python/tests
python -m mypy.stubtest molgfx._engine
```

The workspace default members cover the engine only. Python, WASM, and benchmark
leaves remain part of every `--workspace` command but stay outside the fast
no-selector build.

## Non-goals

MolGFX does not own molecular parsing or analysis, a second selection language,
simulation, docking, a desktop event loop, or viewer camera/input controllers.
Those responsibilities belong to MolFrame or the embedding viewer.

## License

MIT. See [LICENSE](LICENSE).
