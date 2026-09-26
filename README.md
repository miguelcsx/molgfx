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

let mut renderer = Renderer::new()?;
let image = renderer.render_image(&scene, (1920, 1080))?;
image.save("structure.png")?;
# Ok(())
# }
```

`molgfx::camera` builds validated cameras from plain coordinate triples, and
`molgfx::source` is the seam a language binding implements to hand MolGFX
storage it already owns — the Python and WASM bindings use nothing else.

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

# mmCIF lists only the bonds between residues and ligands; inferring the rest
# gives ball-and-stick, licorice and lines their sticks.
structure = molframe.read("structure.cif").infer_bonds()
scene = molgfx.Scene(structure)
scene.add(molgfx.rep.cartoon(target=molgfx.sel.protein()))
scene.add(molgfx.rep.ball_and_stick(target=molgfx.sel.ligands()))

renderer = molgfx.Renderer()
renderer.render_image(scene, size=(1920, 1080)).save("structure.png")
```

Arguments are keyword-only where builders have multiple policies. Python
retains the MolFrame capsule instead of converting coordinates through NumPy.
The public exception hierarchy is `MolgfxError`, `SpecError`, and
`RevisionConflict`; the shipped stubs contain no `Any` escape hatch.

Notebook interaction uses `molgfx.viewer.Viewer`. The AnyWidget sends the
canonical scene JSON, compact BinaryCIF source buffers, and subsequent patches
to the packaged `molgfx-wasm` module, which travels with the widget's own state
so it loads in Jupyter, JupyterLab, VS Code and Colab alike. That module parses the same contract and
renders directly into a browser canvas with WebGPU. Resize remains local to the
browser and no rendered frame pixels travel back through the Python kernel.

## Commands

Scenes can also be authored with a small command language. Molecular
selections are MolFrame queries, handed to MolFrame exactly as written, so
there is one query language and its diagnostics point into your text. Named
selections are written `$name`; a layer is written `@name`.

```text
select pocket, byres (within 5 of resname HEM)
show cartoon, protein
show ball_and_stick radius=0.3 as site, $pocket
color red, chain A           # a rule: chain A is red in every layer
color chain, @cartoon         # a layer's own colour
select pocket, byres (within 8 of resname HEM)   # @site follows the new definition
undo
```

A `Session` resolves names and plans each program into one atomic
`ScenePatch`: every statement applies or none does, and a colour or visibility
change is a local edit that never rebuilds geometry. `show` is idempotent —
asking again for the same form over the same target reuses the layer unless
`duplicate` is given. Errors are typed values with a byte span, the MolFrame
diagnostic code for a query error, and a nearest-spelling suggestion.

```rust,no_run
use molgfx::Scene;
use molgfx::command::Session;

# fn run(structure: &molframe::Structure) -> Result<(), Box<dyn std::error::Error>> {
let mut scene = Scene::from_structure(structure)?;
let mut session = Session::new(&scene);
let outcome = session.execute_text(&mut scene, "show cartoon, protein; color red, chain A")?;
let patch = outcome.patch; // send this to a viewer
# let _ = patch;
# Ok(())
# }
```

In Python the session edits the same live scene a viewer shows, and
`molgfx.viewer.Workbench` adds a command line, history and error panel under
the canvas:

```python
import molframe, molgfx
from molgfx.viewer import Workbench

bench = Workbench(molframe.read("4hhb.cif").infer_bonds())
bench.execute("show cartoon, protein; select heme, resname HEM; show spacefill, $heme")
bench  # type more commands in the page; Tab completes, arrow keys recall
```

Typed commands are available as `molgfx.Command` (`Command.show("cartoon",
"protein", width=2)`), `session.completions(text)` lists what fits at a cursor,
and `session.to_json()` saves names, layers and rules with the targets as
declared, `$name` references included. The browser bindings expose the same
`Session`, compiled from the same Rust grammar.

## Wire contract and interoperability

`SceneSpec` and `ScenePatch` are deterministic JSON values consumed by Rust,
Python, and WASM. Runtime data bindings remain out of band and are identified by
URI and content hash. `molgfx::interop` imports and exports the compatible
MolViewSpec v1 subset as `.mvsj` or `.mvsx`; unsupported nodes produce explicit
diagnostics, and MolGFX metadata is preserved in a namespaced extension.

`molgfx::streaming::DataSource` defines bounded asynchronous provider behavior.
Its scheduler validates priority-preserving batches, cancellation, backpressure,
and shutdown. The existing lower-level residency engine remains an expert API
until source descriptors and provider scheduling are connected end to end.

## Performance invariants

- Coordinates are borrowed from MolFrame and never copied into scene records.
- Atoms and bonds use analytic instanced impostors, not tessellated meshes.
- Culling writes indirect draw counts on the GPU.
- Upload staging and residency are bounded; static assets share one immutable
  arena. The chunk upload path is still being consolidated into backend copy
  batches.
- Picking uses one aligned readback buffer and one mapping operation.
- Structure coordinates, the visual property arena and atom BVHs are shared per
  immutable asset. Packed molecular records, compaction maps, visibility lists
  and indirect arguments are still owned per physical representation, so
  overlapping representations over one molecule each carry their own; sharing
  them by canonical selection is the next ownership change.
- Appearance is separate from molecular records: opacity, material and
  visibility edits write fixed-size uniform state and never repack atoms or
  bonds, at any scene size. Hiding a representation gates its draws and retains
  its resources.
- Semantic interaction channels — selected, hovered, focused, muted, hidden and
  caller-named ones — are one GPU-resident state word per atom, shared by every
  representation over that structure and readable from a visual style.
- WGSL is composed and validated once from `molgfx-shaders`; there is no SPIR-V
  output or second maintained shader dialect.

Use `explain()` and stable hashes to inspect authoring values and renderer plans
without exposing physical handles. Visual explanations identify the typed
bytecode interpreter until specialized WGSL is selected by the renderer.

Neither `SceneSpec` nor the scene manifest carries a hand-maintained version
number. Compatibility is decided by the shape of the data: a document that does
not match the current schema fails to deserialize. Package versions live in
`Cargo.toml` and nowhere else.

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

MIT. See [LICENSE](https://github.com/miguelcsx/molgfx/blob/main/LICENSE).
