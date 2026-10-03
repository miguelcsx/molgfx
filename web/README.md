# MolGFX browser SDK

The official TypeScript host over the MolGFX Rust engine. Import `Viewer` from
`molgfx` or `molgfx/viewer`, and import `molgfx/viewer.css`. React lifecycle
integration is available from `molgfx/react`; `molgfx/anywidget` is the notebook
adapter shipped unchanged in Python wheels.

`Viewer.create(element)` mounts a canvas. `load(file)` or `load(bytes, { name })`
loads molecular input through MolFrame. `execute(program)` edits the authoritative
Session. Call `await viewer.dispose()` during teardown. The host supplies no
Workbench, console, toolbar or file picker.

Build from a repository checkout with `nix develop -c npm ci --prefix web` and
`nix develop -c npm run build --prefix web`. The build copies one shared artifact
set to Python static assets. Documentation imports this same npm package,
letting Next.js emit its wasm asset. The content-hash manifest and archive
checks enforce byte equality and Cargo/Python/npm version agreement.

Verification uses `npm test --prefix web` for strict lifecycle/revision contracts,
`npm run test:browser --prefix web` for real WebGPU, semantic picks, React lifecycle
and measured workloads, and `npm run test:bundler --prefix web` for packed npm
consumers in production Vite and Next builds. No npm publication occurs.
