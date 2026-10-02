# Changelog

All notable changes to MolGFX are recorded here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 a minor release may
change the public API. MolGFX 0.4 needs `molframe>=0.4.0`: publish or install
that first.

## 0.4.0

### Added

- **Compositions in one call.** `Scene.pocket` builds the pocket-and-pose view of
  a ligand; `Scene.ensemble` overlays weighted structures, heaviest most opaque;
  `Scene.difference` colours a bound property and fades small values to context;
  `Scene.auto` chooses default forms from the structure's size.
- **Placed copies.** `Scene.place` draws a structure at an affine transform and
  `Scene.assembly` lays out a biological assembly from `molframe.crystal.assembly`;
  `Scene.add_structure` is public, so a scene can hold several structures from
  Python. The browser receives each distinct source once.
- **Camera paths** (`CameraPath`) and `Renderer.render_camera_path` for converged
  frame sequences along them.
- **Colour by what the structure knows:** a `plddt` confidence metric with the
  four AlphaFold bands, and the aliases `rainbow` and `confidence`.
- Picking returns detached results from a bounded pool, so interleaved picks keep
  their own provenance.
- `CHANGELOG.md`.

### Changed

- The surface-field limit is a per-tier limit, so a publication render reaches
  0.25 Å on large complexes instead of being refused; publication ambient
  occlusion uses 4 rays per sample, measured against 16.
- A frame is complete only when every drawable is resident, not when uploads have
  drained.
- A converged output submits its samples in runs of four and waits for the run
  before last, and growable buffers round up to an eighth of an octave instead of
  a power of two. Measured on 1AON at 1280x720 through `render_image`, peak
  footprint of cartoon fell from 2.50 to 1.12 GB and of spacefill from 2.25 to
  0.83 GB, with no increase in time per output.
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
