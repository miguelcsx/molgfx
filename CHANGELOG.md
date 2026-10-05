# Changelog

All notable changes to MolGFX are recorded here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 a minor release may
change the public API. MolGFX 0.5 needs `molframe>=0.5.0`: publish or install
that first.

## 0.5.0

### Changed

- Require MolFrame 0.5.0 in Rust and Python, with the native Python bridge
  pinned to its published upstream source.
- Cartoon helix and nucleic cross-sections expose elliptical, rounded and
  square profiles, aspect ratio and strand arrow factor across Rust, Python
  and WASM. Source-anchored direction wedges follow live coordinates.
- Ribbon and mesh rendering uses native indexed draws. Requested ribbon
  detail is preserved for large structures; geometry exceeding GPU index
  limits fails explicitly rather than truncating.

### Fixed

- Canonical polymer gaps and closed ribbon end faces preserve source topology.
- Reactivating a resident trajectory initializes previous coordinates at its
  new sample, preventing motion blur from a previous activation.

## 0.4.2

### Added

- Native scene-linear HDR export through the public Rust and Python renderer.
  `HdrImage` preserves half-float highlights above one, exposes encoded OpenEXR
  bytes and streams files without duplicating the full pixel buffer. Python
  `.save()` rejects non-EXR filenames before writing.
- Render profiles expose explicit validated bloom, depth of field, motion blur,
  depth cue, anti-aliasing, backdrop, lighting, shape cues and display overrides
  in Rust, Python and WASM.
- Affine scalar volumes support isosurfaces, voxel-space mesh/dot masks, direct
  transfer, slices and regions in volume-only or molecular scenes. Categorical
  segmentations carry independent labels and styles with generational picks.
- Named session snapshots restore scene state atomically and remain undoable;
  duplicate snapshot, fitting and cell values have been removed.

### Fixed

- Windows x86-64 MSVC builds embed DXC instead of relying on an ambient
  compiler or FXC for dynamic visual-program shaders. Installed-wheel release
  verification explicitly renders through DX12 with the embedded compiler.
- Linux browser verification uses full Chromium with the headed Xvfb
  compositor: headless SwiftShader can complete GPU submissions while
  presenting a blank canvas. Pixel, picking and resize checks remain required.
- Python colour metric typing includes all seven canonical metrics and the
  existing `rainbow` and `confidence` aliases.

## 0.4.1

### Added

- The official canvas-only `molgfx` browser SDK, with viewer, React and
  AnyWidget entry points. Python `Viewer` accepts a Structure, Scene or Session;
  commands and history remain Session-owned. Application panels stay outside
  the SDK. AnyWidget is a normal dependency, loaded lazily by the Python API.
- Atomic current selection: `Command::SetSelection`, Python
  `Command.set_selection`, and `selection QUERY` update the selected overlay
  and `$sel` together, with rollback and undo/redo.
- Explicit Python scene transport through `browser_sources`, `subscribe` and
  `unsubscribe`; reentrant subscription changes survive publication.

### Fixed

- Demand-driven rendering requests skipped frames, pending uploads and
  incomplete temporal exposure. Successful scene synchronization records the
  representation revision, so exposure converges, idles and restarts on motion.

### Distribution

- One content-keyed browser build supplies npm and Python static assets; actual
  archive checks enforce matching versions and bytes. Documentation imports the
  public npm package and lets its bundler emit the WASM asset.
- Python source distributions ship the prebuilt browser runtime; Python builds
  do not require Node or wasm-pack. Release verification covers source rebuilds
  and installed widgets, not only core imports.
- npm release automation uses a separate trusted-publisher workflow; registry
  authorization remains account-owned.

## 0.4.0

### Changed

- **Exact secondary structure.** Preserve all eleven MolFrame states, including
  three-ten/pi/other/PPII helices, isolated beta bridges and bends, across
  palette categories, native Python import, picking, hashes and scene manifests.
  Canonical labels replace the old helix label; invalid labels/codes fail.
  Helix and sheet queries use family semantics; cartoon/rocket profiles retain
  the full assignment. A distinct Paul Tol scientific categorical palette gives
  every state its own colour; reference palettes and PyMOL subtype limits stay
  explicit. Modern mkdssp, rather than Mol*'s older assignment, is the oracle.
- **Secondary palette gallery.** Add named 1AON and 7QPD cases with all four
  secondary-structure review items. `frame_time --manifest FILE --case ID`
  measures the authored gallery commands, with corpus checksum verification.
  Reference engine subtype and palette limits remain review evidence, not
  cross-engine pixel-equality claims.
- **Engine names replace use-case names.** `Quality::Publication`,
  `profile::publication()`, `ImagePurpose::Publication`, `RenderMode::Cinematic`
  and the `molgfx-publication` recipe are now `Converged`/`converged()`; the
  Illustration effect is `ShapeCues`; `RenderProfile::{illustrative,cinematic,
  inspection}` are `shape_cues`/`optical`/`bare`; `LightingEnvironment::documentary`
  is `soft_key`. No aliases remain.

### Added

- **Compositions in one call.** `Scene.pocket` builds the pocket-and-pose view of
  a ligand; `Scene.ensemble` overlays weighted structures, heaviest most opaque;
  `Scene.difference` colours a bound property and fades small values to context;
  `Scene.auto` chooses default forms from the structure's size.
- **Placed copies.** `Scene.place` draws a structure at an affine transform and
  `Scene.assembly` lays out a biological assembly from `molframe.crystal.assembly`;
  `Scene.add_structure` is public, so a scene can hold several structures from
  Python. Each distinct source is encoded once.
- **Camera paths** (`CameraPath`) and `Renderer.render_camera_path` for converged
  frame sequences along them.
- **Colour by what the structure knows:** a `plddt` confidence metric with the
  four AlphaFold bands, and the aliases `rainbow` and `confidence`.
- Picking returns detached results from a bounded pool, so interleaved picks keep
  their own provenance.
- **Camera primitives for hosts.** `Camera.project` and `Camera.ray` map world
  points and pixels with the renderer's conventions; `Scene.frame` and
  `Scene.selection_bounds` frame a selection without changing the scene; the
  arcball, orbit and fly controllers turn abstract input events into camera
  moves, in Rust (`molgfx::controls`), Python and the WebAssembly bindings.
- `Renderer(surface_memory_mib=...)` sets the device memory one implicit-surface
  field may take (384 MiB by default).
- `CHANGELOG.md`.

### Removed

- The application shell: the notebook Viewer and Workbench, console, toolbar,
  file picker, optional `jupyter` extra and embedded documentation viewer.
  Reusable browser hosting returns in 0.4.1 without application UI.

### Changed

- The surface-field limit is a per-tier limit, so a publication render reaches
  0.25 Å on large complexes instead of being refused; publication ambient
  occlusion uses 4 rays per sample, measured against 16.
- A frame is complete only when every drawable is resident, not when uploads have
  drained.
- A converged output submits its samples one at a time and waits for the run
  before last, and growable buffers round up to an eighth of an octave instead of
  a power of two. Measured on 1AON at 1280x720 through `render_image`, peak
  footprint of cartoon fell from 2.50 to 1.12 GB and of spacefill from 2.25 to
  0.83 GB, with no increase in time per output.
- **Peak memory, measured.** A fragment shader that copied a 1 KiB colour-ramp
  table into a local, and a surface shader that held eight corner normals live
  across its field loads, made the Metal driver reserve private memory for every
  thread the GPU can run at once; both now read the table in place and blend the
  normals as they go. A cartoon is also sampled coarser, down to two samples per
  interval, until it fits 400,000 vertices, and its host scratch mesh is
  released after upload. Peak process footprint (`footprint`, Apple M5 Pro,
  macOS 26, `Renderer()` through `render_image`, whole structure in one form):

  | structure | cartoon | spacefill | licorice | surface |
  |---|---|---|---|---|
  | 1CRN, 1280x720 | 353 MB | 363 | 378 | 396 |
  | 4HHB, 1280x720 | 407 | 368 | 387 | 543 |
  | 4HHB, 1920x1080 | 505 | 461 | 480 | 627 |
  | 1AON, 1280x720 | 670 | 481 | 577 | 722 |

  Before, the same renders took 1CRN 354-673, 4HHB 428-820 and 1AON 607-986 MB.
  About 150 MB of every figure is the first Metal render pass, which a bare wgpu
  program also pays; the driver gives much of it back when the renderer is idle
  (a 4HHB cartoon at 1280x720 falls from 430 to 221 MB after three seconds and
  returns to 428 MB on the next render). A surface of a large complex holds two fields while it is built, so its
  peak follows `surface_memory_mib`.
- Module roots declare and re-export only; the ensemble opacity policy has one
  definition in `molgfx-core`.

### Fixed

- `Scene.place` and `Scene.assembly` failed with "Already borrowed" when a viewer
  was attached.
- A volume-segment pick named the first volume even when several existed.
- On OpenGL, every transparent, volume and segmentation draw failed validation:
  wgpu treats a read-only depth attachment there as a write, so the live depth
  could not also be bound. Those passes now read a copy of the opaque depth on
  devices that cannot do both.
- The stub of the engine lacked the reflected operators (`__radd__`, `__rmul__`,
  `__rand__`, `__ror__`), and a broken documentation link failed the release
  check; rustdoc with warnings denied now runs on every push.

### Known limits

- 120 completed 64-sample outputs per second is not reachable on the measured
  hardware (cartoon 16.8 outputs/s on 11,521 atoms); numbers are in `missing.md`.
- Ensemble and difference views, and the generic `compose_*` of `molgfx-semantic`,
  have no command-language form.
- No timeline or segmentation specification, no geometry export (GLB/OBJ/STL), no
  JPEG or WebP export, no sparse surface-field atlas, and no golden-image test.
- An implicit surface of a large complex holds three full-resolution fields: 1AON
  (58,870 atoms) at 0.25 Å takes about 3.5 GB of device memory and 28 s for one
  converged output. A sparse brick atlas is the fix and is not built.
- A continuous colour is set on a representation, not on a query rule.
- Bonds between placed copies are not generated, and a pick in a placed copy
  resolves against that copy's own structure.
