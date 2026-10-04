# MolGFX

[Documentation](https://miguelcsx.github.io/molgfx/)

MolGFX is a semantic molecular-rendering library for Python, Rust, and WebGPU.
It renders structures supplied by [MolFrame](https://github.com/miguelcsx/molframe)
as interactive browser scenes or deterministic native images. MolGFX does not
parse, fetch, dock, simulate, or open an application window.

![MolGFX haemoglobin cartoon, heme, and local pocket](site/public/images/haemoglobin-pocket.png)

## Install

```bash
python -m pip install --upgrade molgfx
```

## Render a structure

```python
import molframe
import molgfx

# Infer bonds so atom-and-bond representations have sticks to draw.
structure = molframe.read("structure.cif").infer_bonds()
scene = molgfx.Scene(structure)
scene.add(molgfx.rep.cartoon(target=molgfx.sel.protein(), color=molgfx.color.chain()))
scene.add(molgfx.rep.ball_and_stick(target=molgfx.sel.ligands()))

molgfx.Renderer().render_image(scene, size=(1920, 1080)).save("structure.png")
```

For native HDR export, retain scene-linear highlights in half-float OpenEXR:

```python
hdr = molgfx.Renderer(profile=molgfx.profile.converged()).render_hdr_image(scene, size=(1920, 1080))
hdr.save("structure.exr")
```

`HdrImage` exposes `width`, `height`, `exr_bytes()`, `rgba16f()` and
`quality_json()`. HDR capture completes the profile's exposure without baking
in bloom, exposure, tone mapping, display conversion or screen overlays.
Python `.save()` accepts only `.exr` filenames and raises `ValueError` before
writing any other format. Rust uses `HdrImage::save_exr` and also supports an
explicit camera through `Renderer::render_hdr_image_with_camera`.

Targets are MolFrame queries, and a small command language works around them
through `molgfx.Session`:

```text
show cartoon, protein
select pocket, byres (within 5 of resname HEM) and protein
show licorice, $pocket
color orange, $pocket
```

The Rust engine opens no window and owns no application panels. This repository
also supplies the official browser host: a canvas, navigation and semantic picks,
not a Workbench, console, toolbar or file picker. `Session` remains the sole
command and undo/redo authority. Python notebooks use this same frontend through
AnyWidget, a normal package dependency.

```python
session = molgfx.Session(structure)
session.execute("show cartoon, protein")
viewer = molgfx.Viewer(session)
viewer  # Display in Jupyter.
```

## Browser SDK

```bash
npm install molgfx
```

```typescript
import { Viewer } from "molgfx";
import "molgfx/viewer.css";

const viewer = await Viewer.create(document.getElementById("scene")!);
await viewer.load(await fetch("structure.cif").then(r => r.arrayBuffer()), { name: "structure.cif" });
viewer.execute("show cartoon, protein");
// On host teardown:
await viewer.dispose();
```

The one npm package exports `molgfx`, `molgfx/viewer`, `molgfx/anywidget` and
`molgfx/react`. The React adapter owns viewer mount/dispose, not molecular state.
Public TypeScript declarations never expose the raw wasm ABI.

```tsx
"use client";
import { MolGFXViewer } from "molgfx/react";
import "molgfx/viewer.css";

export function Protein({ bytes }: { bytes: Uint8Array }) {
  return <MolGFXViewer structure={bytes} name="protein.cif"
    style={{ width: "100%", height: 480 }}
    onError={error => console.error(error.message)} />;
}
```

Keep the input byte object stable between unchanged React renders. The adapter
reuses its Viewer for structure updates and disposes it on unmount; it does not
duplicate Scene, Session or rendering semantics.

## Shared frontend build

```bash
nix develop -c npm ci --prefix web
nix develop -c npm run build --prefix web
nix develop -c npm test --prefix web
nix develop -c npm pack ./web --pack-destination web
```

`web/scripts/build.mjs` runs wasm-pack into ignored `web/generated`, then emits
the official package into ignored `web/dist` and copies identical artifacts to
Python static assets. The documentation site imports the public npm package;
Next.js emits its WASM asset rather than loading a private runtime directory.
A content-hash manifest and archive-level checks prove that npm tarballs and
Python wheels carry identical runtime bytes. Build before running maturin from
a source checkout; PyPI source distributions include the prebuilt runtime and
need Python >=3.12, Rust and dependency fetching, but no Node, wasm-pack or
adjacent MolFrame checkout. Cargo, Python and npm versions must match.
Native wheel builds repair external libraries; release gates rebuild the source
archive and exercise fresh installed wheels before publication.

Python and npm have separate release workflows. Published versions are immutable;
release 0.4.1 supersedes the existing PyPI 0.4.0 package. npm trusted publishing
requires the package owner to authorize this repository and release workflow.
Until the first registry publication, the docs use the public package through a
local npm dependency, exercising the same exports and bundler asset resolution.

[Writing queries and commands](https://miguelcsx.github.io/molgfx/docs/commands/queries)
walks through both, and the
[MolFrame query reference](https://miguelcsx.github.io/molframe/docs/query-language/)
lists every keyword.

## Status

Beta. Public APIs are available through the `molgfx` facade; MolFrame provides
structure input and molecular selection semantics.

## License

[MIT](LICENSE)
