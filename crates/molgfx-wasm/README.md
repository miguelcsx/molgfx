# molgfx-wasm

Thin browser bindings for molgfx over WebGPU.

Part of [MolGFX](https://github.com/miguelcsx/molgfx), a semantic, GPU-native molecular rendering engine. Most callers want the `molgfx` facade crate rather than this one.

## The browser runtime

This crate is the only browser package. `src/` holds the wasm bindings;
`js/` holds the thin viewer host (canvas, camera, picking, command console)
with two entry points over one core:

- `widget.js`: the [anywidget](https://anywidget.dev) module that
  `molgfx.viewer.Viewer` and `Workbench` load in Jupyter and Colab.
- `mount.js`: `mount(element, { structure, program })` for a plain page, with
  commands running in the page. The documentation site's demo uses it.

One build produces `js/dist/` (both entries, `widget.css`, the wasm pair and
`runtime-manifest.json`); consumers copy it whole:

```bash
cd crates/molgfx-wasm/js && npm ci
node scripts/build.mjs --out ../../../python/molgfx/viewer/static --out ../../../site/public/runtime
```

`--dev` builds unoptimized wasm; `--no-wasm` rebuilds only the TypeScript.
