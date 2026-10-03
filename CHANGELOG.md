# Changelog

All notable changes to MolGFX are recorded here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 a minor release may
change the public API. MolGFX 0.4 needs `molframe>=0.4.0`: publish or install
that first.

## 0.4.0

### Changed

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

- The application shell: the notebook `Viewer` and `Workbench`, the TypeScript
  viewer host with its console and plain-page `mount()`, the `jupyter` extra, the
  example notebook and the documentation home page's embedded viewer. The
  specification keeps the engine free of an application shell (§4.4, ADR-0012);
  a notebook, page or application is built on the bindings and the camera
  primitives. The code is set aside outside the repository for later review.

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
