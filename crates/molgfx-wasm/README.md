# molgfx-wasm

Thin browser bindings for molgfx over WebGPU.

Part of [MolGFX](https://github.com/miguelcsx/molgfx), a semantic, GPU-native molecular rendering engine. Most callers want the `molgfx` facade crate rather than this one.

## What it exposes

A page builds its own viewer on these; the crate opens no window and draws no interface.

- `Scene`, `SceneSpec`, `ScenePatch`: the declarative scene and its revision-checked patches, shared with Rust and Python.
- `Renderer`: renders a scene to a canvas the page provides, with completed-frame telemetry and detached GPU picking.
- `Camera`, `ArcballController`, `OrbitController`, `FlyController`: the camera, its controllers driven by abstract input events, and the mapping between world points and pixels (`project`, `ray`). `Scene.frame` returns the camera that fits a selection.
- `Session`: the command language over a scene.

## Build

```bash
wasm-pack build crates/molgfx-wasm --target web --release --out-dir pkg
```

`pkg/` holds the module, the `.wasm` file and `molgfx_wasm.d.ts`.
