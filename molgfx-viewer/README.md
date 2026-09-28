# molgfx-viewer

The MolGFX notebook widget: a TypeScript frontend that turns the
`molgfx-wasm` browser runtime into an interactive [anywidget](https://anywidget.dev)
view for Jupyter and Colab -- canvas, camera controls, picking and the
optional command console.

This is browser code, not Python: it has no place under `python/`, which is
why it lives here as its own project. Its only relationship to Python is its
build output, which lands in `python/molgfx/viewer/static/` for the wheel to
package; nothing here is imported by Python directly.

## Layout

```
src/
  core/        types, config constants, DOM helpers, lifecycle, model I/O
  runtime/      loading the wasm runtime, building/patching the wasm scene
  render/       camera math and the render loop
  interaction/  pointer, wheel and keyboard input
  console/      the optional command console (elements, completion, history)
  viewer.ts     composition root: wires the above into one mounted view
  index.ts      the anywidget entry point
```

## Building

Part of the Nix dev shell (`nix develop`); the pinned Node toolchain comes
from there rather than a system install.

```bash
cd crates/molgfx-wasm && wasm-pack build . --target web --out-dir ../../molgfx-viewer/wasm --out-name molgfx_wasm
cd molgfx-viewer && npm ci && npm run build
```

`npm run build` typechecks against the wasm-pack bindings in `wasm/` (git-ignored, rebuilt above) and then bundles `src/index.ts`, copies `src/widget.css`, and copies the wasm runtime into `python/molgfx/viewer/static/` -- the same directory a Python build or test run reads from. Skipping the `wasm-pack` step still produces a `widget.js`, just one that has nothing to load at notebook runtime; run it first for a working viewer.
