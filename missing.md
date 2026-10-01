# Missing: the verified MolGFX gap list

This list is the actionable companion to `mapping.md`. It records the current
state after the MolFrame topology work, MolGFX representation/API work, the
cross-engine source audit, and the browser/PyMOL/Mol* render smoke pass.

`[x]` implemented and checked at the stated boundary · `[~]` implemented in a
narrower form or not yet verified against the shared fixture · `[ ]` open.

Priority:

- **P0** changes the structural picture or makes an existing public path wrong.
- **P1** missing engine capability or interaction contract.
- **P2** quality, scale, or measured performance parity.
- **P3** broader reference-engine feature, application workflow, or convenience.

A source audit is not a substitute for a rendered comparison. Every `[~]` item
names the missing fixture, measurement, or semantic boundary.

## Current execution checkpoint — 2026-09-30

[plan.md](plan.md) preserves the complete agreed scope, ownership, sequence and
acceptance criteria, including Gemmi → MolFrame. Historical checks below remain
evidence for their specific boundary, not a green gate for the whole current tree.

- [~] **P0** HighestFixed and canonical quality metadata are implemented across
  Rust images/HDR, Python and WASM. Native Python observed High/64 completed
  samples; WebGPU observed immutable completed metadata after resize. **Open:**
  complete scene fidelity, effective spacing and full-resolution performance.
- [x] **P0** Full-residency certification no longer reads only upload tickets:
  `full_residency` also requires every planned slot to have synced and no
  queued trajectory interpolation (`GpuScene::pending_drawables`, regression
  test `a_synced_scene_is_fully_resident_and_an_unsynced_slot_is_not`).
  **Open:** declared placements and provider work that have no slot yet are not
  visible to this check, and the surface field cap below still makes large
  surfaces correctly incomplete.
- [~] **P0** Shared exposure recording, distinct uniform ranges and tracked fences
  exist. **Open:** finish sequence/concurrency and whole-corpus acceptance; no
  reused samples, stale-camera output or subsample-as-frame accounting.
- [~] **P0** Actual per-pass profiling, coverage, persistent scratch and timestamp
  absence reasons exist. Cancellation/stale-slot changes are in the tree.
  **Open:** final observed real-device verification of those latest changes;
  invalid_order/stale/unresolved remain null durations, not measured zero.
- [~] **P0** Registered shared-corpus harness and real native, Mol* and PyMOL
  adapters exist. Small-extent execution evidence is recorded below. **Open:**
  finish maximum-quality reference recipes/corpus, resolve semantic differences,
  then run full-resolution cold/warm/repeated acceptance. Interrupted large ray
  cases are unfinished measurements, not missing reference capabilities.
- [ ] **P2** Prove 120 completed outputs/s and p95 <= 8.333333 ms at maximum
  effective quality, without lowering resolution, samples, geometry or effects.
  Report p99/backlog/allocations/RSS and per-recipe settings. Offscreen completion
  cannot certify 120-Hz presentation on the observed 30-Hz display.

- [x] **P1** Detached picks own their readback buffer and captured page
  generations (`PendingPick`, at most four in flight, explicit
  `InFlightExhausted`); `finishPick` takes the readback, so a later pick, frame
  or scene edit cannot change which entity it resolves. A volume-segment pick
  names a volume only when exactly one exists. **Open:** a `SegmentationSpec`
  domain so categorical picks can name their own segmentation.
- [x] The pocket-and-pose composition (focus subject, interaction shell,
  orienting shell, pocket surface, local solvent, far context) is one declarative
  call in every surface: `Scene::add_pocket`, the `pocket` command, and Python
  `Scene.pocket(..., style=PocketStyle(...))` / `Command.pocket`. **Open:** a
  real-protein golden image and a measured frame budget for the composition.

### MolFrame / Gemmi

- [x] Reciprocal vectors/d-spacing and exact reflection centricity, systematic
  absences and epsilon classification: MolFrame CellTransform/ReflectionSymmetry,
  borrowed Miller iteration, shared MTZ calculation and Rust/Python facade.
  **Observed:** differential against local Gemmi across all 530 Hall settings,
  386,370 reflections and 2,187 spacing checks; maximum relative error 5.7e-16.
  The temporary consumer was /tmp/molframe-gemmi-reciprocal-smoke.py.
- [~] Integration gate for this slice: targeted formatting and Clippy for
  molframe-xtal/molframe-py, all targets with -D warnings, passed. **Open:**
  full gate, benchmark assessment and publication/pin verification; this is not
  general Gemmi parity or a speedup claim.
- [x] X-ray form factors (International Tables, H–Cf) and direct structure
  factors over the full space group with occupancy and isotropic/anisotropic
  displacement, from a structure's own cell and symmetry, in Rust and Python.
  **Observed:** differential against Gemmi 0.7.5 on P 21 21 21, P 1 21 1
  (anisotropic) and I 2 2 2, agreement ≈1e-5 relative. A PDB `CRYST1` space
  group is now kept and resolved to symmetry operations (it was dropped).
  **Open:** anomalous dispersion, neutron/electron tables, ions.
- [ ] Continue density affine/sampling consistency and the remaining
  reciprocal-processing, density-from-model and FFT workflows after
  source audit. Preserve existing CIF/PDB/BCIF, symmetry, crystal neighbors,
  maps and reflection I/O; do not introduce Gemmi as a product dependency or
  copy its implementation into the renderer.

- [x] **P0** Close-camera analytic impostor payloads remain valid after proxy
  clipping. Every vertex supplies flat data in the atom, bond, point, primitive,
  instance, ligand, overlay and affected shadow paths. **Check:** same-camera
  browser comparison removed the observed black triangle with empty runtime
  and page errors; 381 render tests and 25 shader tests passed. Cross-engine
  image parity and the vertex-work performance cost remain unmeasured.

## Phase A — MolFrame topology

- [x] **P0** Standard-residue bonds come from a built-in constant component
  table, including amino acids, nucleotides, water, aromatic ring metadata,
  and known double bonds. **Owner:**
  `../molframe/crates/molframe-chem/src/standard_bonds.rs`.
  **Check:** `cargo test -p molframe-chem` (60 tests observed).
- [x] **P0** Polymer links are inferred for peptide C–N and nucleic O3′–P
  connections. **Owner:** `molframe-chem/src/bonds.rs`.
  **Check:** MolFrame chemistry tests and facade read tests.
- [x] **P0** Remaining bonds use a sorted spatial grid with element thresholds,
  no H–H edges, alternate-location compatibility, and metal coordination
  metadata. **Owner:** `molframe-chem/src/bonds.rs`.
- [x] **P0** File bonds are retained before inferred bonds; inferred distance
  bonds are marked and omitted from CIF output. **Owner:** MolFrame facade,
  CIF/PDB readers and writers.
- [x] **P0** PDB HELIX/SHEET and CIF secondary-structure categories are parsed
  into the source rows. **Owner:** `molframe-pdb`, `molframe-cif`.
- [x] **P1** DSSP-like assignment uses shared spatial candidates and a fallback
  geometric classifier. **Owner:** `molframe-chem/src/secondary.rs`.
- [x] **P1** CA-only Zhang–Skolnick-style fallback fills remaining unknown rows.
  **Owner:** `molframe-chem/src/secondary.rs`.
- [x] **P1** Direct caller-created `Structure`/topology values do not run the
  facade read enrichment automatically. The explicit `molframe::perceive`
  boundary applies file → perception → fallback semantics on demand and shares
  the coordinate column. **Owner:** MolFrame facade/core boundary.

## Phase B — MolGFX topology consumption

- [x] **P0** `SourceBond` carries order, aromatic, and metal fields, and the
  GPU bond record carries order/flags without changing coordinate ownership.
  **Owner:** `crates/molgfx-core/src/structure/source.rs`, `gpu/`.
- [x] **P0** File secondary structure is preserved through asset creation and
  placement. **Owner:** `crates/molgfx-core/src/dataset/asset.rs` and
  `structure/`.
- [x] **P0** MolFrame facade reads use file → perception → CA fallback
  precedence before MolGFX receives the topology. **Check:** facade tests and
  core asset tests.
- [x] **P0** Count-based acceptance for the shared fixtures: a protein, a
  nucleic-acid and a ligand input are read and their file-vs-inferred bond
  counts and secondary-structure rows asserted without a renderer, which is the
  number the cross-engine comparison must agree on before any pixel is
  compared. **Owner:** `crates/molgfx-core/src/structure/source_tests.rs`.
  **Check:** `file_and_inferred_bond_counts_are_deterministic_across_the_shared_fixtures`.
  **Open:** the same numbers read in Mol* and PyMOL on identical input, which
  needs those engines and the fixture files present.
- [ ] **P1** Symmetry-mate bond generation and bond picking across instanced
  units. **Owner:** `crates/molgfx-core` (instance topology boundary), not
  MolFrame: the upstream half already exists as
  `molframe_xtal::AssemblyView::neighbors`, which returns
  `AssemblyNeighbor { first_instance, first_atom, second_instance, second_atom }`
  for every pair within a cutoff (`../molframe/crates/molframe-xtal/src/assembly_spatial.rs`),
  and `molframe_xtal::collect_crystal_neighbors` covers bare crystal mates.
  What is missing is the consumer: `SourceTopology`/`BondGpu` carry no instance
  index, and the typed `SceneSpec.assembly.instances` state is not yet lowered to
  molecular placements. Adding it needs an `InstancedBond` row and an instance
  lane on the packed bond record, both on the MolGFX side.

## Phase C — representation fidelity

- [~] **P0** Cartoon spline/ribbon quality, radial sides, and SS-aware profiles
  are implemented. **Open:** exact helix/sheet arrow, nucleotide block/ring,
  gap, and quality-tier parity against Mol* and PyMOL fixtures. The terminal
  strand arrow tapers from its shoulder to a pointed tip and the trace gap is
  one named constant, but neither is measured against a reference render.
  **Owner:** `crates/molgfx-geometry`, `crates/molgfx-render/src/scene_gpu/ribbon_slot.rs`.
- [x] **P0** Ball-and-stick/spacefill forms lower and render analytic atoms;
  file/inferred bonds are packed for BallAndStick, Licorice, and Lines.
  **Check:** render smoke cases and geometry packing tests.
- [x] **P1** Multi-bond geometry carries single/double/triple order and
  endpoint-offset variants, with aromatic and metal dash cadence in the shared
  bond shader. **Owner:** `crates/molgfx-shaders/src/wgsl/include/bond/`.
  **Check:** `the_molecular_bond_pipeline_displaces_every_packed_multi_bond_variant`
  (shader composition), `a_multi_bond_packs_one_strand_per_order_with_sequential_variants`
  and `each_multi_bond_strand_keeps_its_own_variant_index` (packing and record),
  plus a native render of a C=O/aromatic fixture in which the double bond draws
  as two strands. **Open:** a cross-engine visual golden.
- [~] **P0** Licorice uses the shared bond capsule path. **Open:** exact
  junction-cap, half-bond seam, and default-radius parity. The default bond
  radius is now 0.25 Å, the reference value.
- [~] **P0** Lines use screen-space quads and render topology bonds. Lone-atom
  cross markers are implemented end to end: packing leaves a positive point
  radius only on selected atoms without a bond
  (`scene_gpu::record_pack::hide_bonded_line_atoms`), and the point shader
  draws the Mol*/PyMOL cross stroke at the line width against the shared
  four-atom fixture (`line_crosses_remain_only_for_atoms_without_selected_bonds`
  plus a browser render of the three-atom lone fixture). **Open:** line
  anti-alias policy and reference width parity.
- [x] **P1** Typed and command-addressable forms exist for Cartoon, Backbone,
  Trace, Tube, Putty, BallAndStick, Spacefill, Licorice, Lines, Points, Dots,
  Surface, NucleicAcid, Bases, BasePairs, and Glycan. **Owner:**
  `molgfx-scene/src/representation`, `molgfx-command/src/ir/form.rs`.
- [~] **P1** Labels, measurements, volumes, and segmentation have typed API
  and render passes. `label`, `distance`, `angle` and `dihedral` are command
  verbs with parse/print round-trip and undo, and `volume` is now one too: a
  curated command carrying the same portable descriptor the scene wire format
  uses, with undo coverage. **Open:** a `segment` command (the scene has a
  volume domain but no declared segmentation domain to author against), and
  shared protein/ligand/density fixture coverage. **Check:**
  `volume_declares_a_density_grid_and_undo_removes_it`,
  `volume_rejects_a_malformed_specification`.
- [x] **P1** Colour is one rule pipeline. A scheme is a tag, an optional packed
  colour and the arena column it reads; the base scheme and every entry of a
  selection-scoped overlay resolve through the same shader function. Schemes:
  element, uniform, continuous ramp, **category** (chain, entity, molecule type,
  residue name, residue, secondary structure, each on a named palette, optionally
  carbon-only for carbon-by-chain) and **metric** (occupancy, temperature factor,
  formal charge, hydropathy, chain position, solvent-accessible area). Categories and metrics are columns
  the structure derives for itself and the scene binds on first use, so they
  need no shader path of their own and no chain cap: the earlier three-bit
  chain/residue/secondary indices in the atom record are gone. Ramps hold up to
  sixteen stops and reach the GPU as a 256-entry lookup table shared by atom
  colours and surface scalar overlays. Ten palettes and thirty-odd ramps (each
  also reversed with `_r`) are catalogued once in `molgfx-scene` and `molgfx-core`.
  **Check:** `color::*`, `engine::color_scheme_tests`, the real-device
  `gpu_color_scheme`, and native renders of chain, carbon-by-chain, molecule
  type and hydropathy. **Open:** colour by electrostatics and by distance to a
  volume, both of which need a computed field the engine does not derive yet;
  the eleven-state secondary-structure palette needs MolFrame to widen its
  five-state vocabulary (`molframe-core/src/secondary.rs` is
  `Unknown|Coil|Helix|Strand|Turn`), which would also change MolGFX's mirror
  enum, the `"unknown".."turn"` wire labels, and the selection predicates
  (`helix`/`sheet`/`coil`). SASA is now a metric (`AtomMetric::Sasa`, Shrake–Rupley through
  `molframe::surface`, recomputed because it follows the live coordinates) and
  the named colour table is 42 names.
- [~] **P3** `Beads` (one sphere per residue) is now a public form end to end
  (scene, command, facade, Python). Of the six requested extra shapes, three are
  reachable today through the path that already expresses them, and adding a
  second spelling would be a redundant alias rather than a capability:
  - **Blob surface** is `surface(style="soft_union")` in all three languages,
    and the blend span is now a real control rather than a shader constant:
    `RepresentationParams::blob_spread` (default 2.0 Å, zero = the raw
    nearest-atom union), threaded through the scene builder, the command IR
    (`NonNegative`), Python (`blob_spread=`), and the wire (`params[13]`,
    width 15→16). The fragment trace and the impostor ray-bound both derive from
    it, so they cannot disagree. **Check:** the field is validated in
    `RepresentationFormSpec`, `surface_round_trips_and_names_its_soft_union_style`,
    and `soft_union_is_explicit_and_keeps_the_exact_surface_path`; a device
    render at spans 0/0.6/2.5 differs by 138k/263k bytes.
  - **Unit cell** is implemented as guide geometry, not a representation alias.
    `AssemblySpec.unit_cell` is typed `SceneSpec` state; `SetAssembly` validates,
    applies, inverts, and rebinds exactly twelve core guide handles per owner. The
    `assembly {...}` command reaches the same path from Rust and Python. **Check:**
    `assembly_unit_cell_rebinds_to_exactly_twelve_guides`,
    `assembly_unit_cell_reaches_guide_geometry_and_undo_removes_it`, and the
    Python `test_assembly_unit_cell_is_scene_state_and_undoable`.
    Constructor and deserialized validation share core cell geometry checks,
    including impossible angles and translated boundary collapse.
    `assembly null` removes the cell; remove/undo/redo is covered in Rust/Python.
  - **Plane** is caller-authored guide geometry with typed `PlaneSpec` state.
    `Scene::add`, `PatchOperation::{AddPlane,RemovePlane}`, inversion and
    lowering emit exactly four analytic guide segments; the `plane {...}`
    command reaches the same path and is undoable. **Check:**
    `plane_add_and_remove_rebinds_exactly_four_guides` and
    `plane_reaches_guide_geometry_and_undo_removes_it`.
    Core and scene share normalized-frame and representable-boundary validation.
    Exact skew corners/style survive remove/inverse; invalid geometry leaves
    semantic state and native guides unchanged (`overlay/planes_tests.rs`).
  - **Ellipsoid (ADP)** is implemented end to end. MolFrame parses
    `_atom_site_anisotrop` (mmCIF, U- and B-forms) and PDB `ANISOU`, exposes
    `Structure::anisotropy()`, and MolGFX carries the sparse `[U11..U23]`
    tensors into `SourceTopology::anisotropy`; `EllipsoidSpec` is an overlay
    scene item (`PatchOperation::{AddEllipsoids,RemoveEllipsoids}`, inverse,
    `ellipsoid::adp` builder, `Scene.add`, Python `molgfx.ellipsoid.adp`) whose
    lowering emits one `AnisotropicEllipsoid` per selected tensor-bearing atom,
    skipping absent and non-positive-definite tensors. **Check:**
    `anisotropic_tensors_reach_the_atom_that_carries_them`,
    `an_adp_overlay_emits_one_ellipsoid_per_tensor_bearing_atom`,
    `an_isotropic_structure_emits_no_ellipsoids`,
    `an_ellipsoid_overlay_rejects_a_nonpositive_scale`.
  - **Orientation** and **Polyhedron** are genuinely absent: the first needs an
    oriented-glyph primitive (`PointGlyph` is disc-only; `Particle` carries an
    orientation but is not on the form pipeline), the second needs
    `coordination_hull` wired to a mesh-lowering path.

## Phase D — picking and marking

- [x] **P0** Resolved atom picks carry structure/dataset/chunk/revision,
  model/entity, label/auth chain and atom names, residue identity, altloc,
  element, occupancy, B-factor, formal charge, position, secondary structure,
  and a stable label. **Owner:** `molgfx-scene/src/scene/inspect.rs` and the
  WASM contract. **Check:** API pick test plus browser JSON output.
- [x] **P0** Selected, hovered, focused, muted, and hidden state bits are
  consumed by the shared atom/bond/visual shaders. **Check:** shader tests and
  browser selected/hovered/cleared interaction smoke.
- [x] **P0** Browser local-file click applies replace, Shift-add, Alt-remove,
  and Escape-clear through a scene patch and publishes one typed interaction
  event. The patch selects the picked residue while preserving the atom pick
  report. **Owner:** `crates/molgfx-wasm/js/src/viewer.ts`.
- [~] **P0** Marker edge outline. Every G-buffer form folds its strongest
  marker (selected > focused > hovered) into the material payload
  (`visual/marker.wgsl`) and the lighting pass draws a two-pixel edge from it
  (`deferred/marker_edge.wgsl`), with no extra attachment. Checked by rendering
  a selected residue natively. **Open:** ghosted occluded edges need a mask of
  fragments that lost the depth test, i.e. a second draw of marked geometry
  without depth test (Mol* does exactly this); transparent geometry keeps the
  analytic rim because it never reaches the G-buffer.
- [~] **P1** Hover is latest-only and local, and now answers from the previous
  readback while the pointer and the view are unchanged
  (`render-loop.ts` `#hoverMemo`). **Open:** a cached low-resolution picking
  buffer (Mol* `pickScale` 0.25) with one update per frame, so a moving pointer
  over a still scene also costs no readback.
- [x] **P1** Static and dynamic bond picks resolve to source endpoints,
  topology revision, aromatic/metal flags, static order, and dynamic weight;
  renderer depth ordering remains the atom-over-bond precedence by construction.
- [x] **P2** `ResolvedPick` is one documented, internally tagged union with a
  discriminator per kind: `atom`, `bond`, `label`, `measurement`,
  `volume_segment` and `non_atom`, and the superseded externally tagged shape is
  rejected. Label, measurement and volume-segment picks now resolve to the scene
  item they name instead of the renderer's own record; a measurement also has
  its own `EntityKind::Measurement` GPU namespace so it can no longer collide
  with an annotation's storage row. **Check:**
  `resolved_picks_serialize_with_a_pick_discriminator_and_round_trip`,
  `a_label_pick_resolves_to_the_annotation_the_scene_owns`,
  `a_measurement_pick_resolves_to_its_kind_and_arity` and
  `a_volume_segment_pick_resolves_to_its_volume_and_label`.
  **Open:** measurement *value* is still resolved by the renderer, not carried
  in the pick.

## Phase E — rendering quality and scale

- [~] **P1** Temporal resolve, jitter, OIT, AO, shadows, and optional DOF,
  bloom, and motion blur are in the render graph. The convergence contract is
  stated and tested once: a clean frame starts from zero history, four samples
  converge it, and a scene edit or camera motion restarts the count rather than
  letting a stale prefix satisfy it. **Check:**
  `convergence_is_a_four_sample_budget_reached_in_order_and_restarted_by_any_change`.
  **Open:** AO *reuse* across a sample budget and the marker-only flicker case
  need a real-device frame comparison, not a state test.
- [x] **P1** Fog/depth cue is a dedicated `DepthCue` profile input with
  finite ordered-distance and unit-strength validation. It lowers through
  `PresentationEffect::DepthCue` into `FrameUniforms.depth_cue` and the
  deferred view-space cue path without changing legacy illustration semantics.
  **Check:** render 369-test suite and API 125-test suite pass in this pass.
  **Open:** no same-input browser fog fixture or cross-engine distance golden.
- [~] **P1** FXAA is fused into the tonemap pass (`FXAA_ENABLED` pipeline
  constant, edge-tangent blend on encoded luma). It is no longer implicit: the
  presentation profile carries an `AntiAliasing` layer, the scene-facing
  `RenderProfile` has `with_edge_smoothing`, and Python exposes
  `profile.with_edge_smoothing(enabled=...)`. Left unset, the realtime tiers
  smooth and the converged tiers do not, which is unchanged behaviour.
  Compared on a native render of 4HHB. **Open:** SMAA (three passes and the
  area/search tables), and a cross-engine AA comparison. **Check:**
  `a_stated_anti_aliasing_layer_wins_over_the_tier_default`,
  `edge_smoothing_is_unset_until_the_caller_states_it`.
- [~] **P1** Adaptive tiers now drive surface field spacing (0.75 / 0.5 / 0.25 /
  0.25 Å) and ribbon samples per interval (3 / 5 / 8 / 8) through one
  `TierDetail` value; a tier move re-derives only the geometry that reads it.
  Atom-count cap: `High` through 10,000 atoms, `Standard` through 100,000,
  `Reduced` through 500,000, and `Minimal` above that. **Check:** the render
  adaptive tests cover the bands and monotone scene shrinking. **Open:** the
  upload budget remains fixed at engine construction. Preserve live tickets and
  fences when adding budget changes; do not rebuild away in-flight ownership.
  HighestFixed bypasses adaptive atom-count degradation, but full-detail field
  residency and reference quality acceptance remain open.
- [ ] **P2** Sphere stride-prefix LOD with Bayer/dither fade and GPU Hi-Z
  occlusion. Keep the current indirect culling path; do not add per-atom draw
  calls or CPU coordinate copies.
- [ ] **P2** Multi-scale SSAO thresholds and a measured quality/performance
  matrix.
- [x] **P2** Automatic size-based presets: `preset::auto_representations` and
  `Scene::add_auto` classify by polymer residues (small < 10, medium < 5 000,
  large < 30 000, huge) and pick cartoon / nucleic ribbon / atomic detail /
  Gaussian surface per class; solvent is never drawn. Requires the MolFrame
  component-role column, now filled from built-in tables on read
  (`molframe-chem/src/standard_components.rs`). Exposed three ways now: the Rust
  `Scene::add_auto`, the `auto` authoring command (layers registered under
  their form names), and `molgfx.Scene.auto()` in Python.
  **Check:** `preset::tests`, `auto_adds_the_size_appropriate_default_layers_and_registers_them`,
  `auto_round_trips_through_its_canonical_text`, and
  `test_auto_draws_the_default_forms_and_returns_typed_ids`.
- [ ] **P2** Wasm release cold-start and large-scene benchmark evidence.
  **Owner:** `molgfx-bench` and the browser runtime build.
- [~] **P3** Image export is a deterministic native PNG (`Renderer::render_image`,
  `Image.save`) and a frame sequence (`Renderer::render_sequence`, Python
  `Renderer.render_sequence(scene, size=..., fps=..., frames=...)`), which
  streams bounded frames and returns them in submission order. **Open:**
  JPEG/WebP, detached-image lifecycle and Mol* ImagePass/PyMOL ray-setting parity.
  Movie encoding deliberately belongs to the caller. **Historical check:** a
  Python run returned three 64×48 PNG frames; this alone did not prove convergence.

## Comparison and verification backlog

- [x] **P1** Temporary three-engine harness created and exercised common atom,
  bond, cartoon, line, surface, label, point, and quality cases. PyMOL and
  MolGFX used the same temporary PDB; Mol* browser cases used the checked-in
  browser fixtures. Results and blocked network/native paths are recorded in
  `mapping.md`.
- [~] **P0** Same-input comparison uses the registered parity CLI and hashed
  corpus. **Open:** resolve topology/SS differences, complete all maximum-quality
  recipes and full-resolution runs. Small smoke fixtures do not certify parity;
  raw pixel equality across different algorithms/defaults is not acceptance.
- [x] **P1** Browser runtime release build and TypeScript typecheck completed
  with Nix; local selection/hover/clear interaction and rich JSON pick were
  observed without browser errors.
- [ ] **P2** Commit permanent visual golden tests only after the same-input
  fixture and camera contract exists.

## Parity ledger against Mol* 5.11 and PyMOL 3.1

Derived from a source inventory of both trees, not from memory. `[x]` present
and exercised, `[~]` present in narrower form, `[ ]` absent. Every `[ ]` is a
gap a user of either engine will notice. Priority follows the same P0–P3 scale.

### Representations

| Capability | Mol* | PyMOL | MolGFX |
|---|---|---|---|
| Cartoon (helix / sheet arrow / loop) | `[x]` | `[x]` | `[~]` proportions now follow the reference half-extents; nucleotide block/ring, gap dashes, direction wedge open (P0) |
| Backbone / trace / tube / putty | `[x]` | `[x]` ribbon | `[x]` |
| Ball-and-stick, sticks, spacefill, lines, points | `[x]` | `[x]` | `[x]` lone-atom cross and stick junction seam open (P0) |
| Multiple / aromatic / zero-order bonds | `[x]` | `[x]` | `[~]` shader dashes exist and every analytic depth writer now ranks coincident fragments by entity (deterministic junction); no cross-engine golden (P1) |
| Molecular / SAS / vdW / Gaussian surface | `[x]` | `[x]` | `[x]` slow, see Performance (P0) |
| Surface carving / clearing by selection | `[ ]` | `[x]` | `[ ]` (P2) |
| Surface wireframe / mesh / dots | `[x]` | `[x]` | `[x]` |
| Blob surface | `[x]` | `[ ]` | `[x]` `soft_union` with a real `blob_spread` (0 Å = exact union), lowered in all three languages |
| Gaussian volume (density as direct volume) | `[x]` | `[ ]` | `[ ]` (P2) |
| Ellipsoids (ADP) | `[x]` | `[x]` | `[x]` MolFrame parses ANISOU/`anisotrop`; `EllipsoidSpec` overlay lowers one `AnisotropicEllipsoid` per tensor-bearing selected atom, exposed in Rust and Python |
| Orientation glyph, polyhedron, plane | `[x]` | `[ ]` | `[~]` public planar outline renders and picks as a Guide; orientation and polyhedron fidelity remain open (P3) |
| SNFG carbohydrate symbols and links | `[x]` | `[ ]` | `[~]` glycan ribbon, no SNFG glyphs (P2) |
| Residue beads / coarse | `[x]` | `[ ]` | `[x]` new |
| Atom labels (text on atoms, by property) | `[x]` | `[x]` | `[~]` free labels only (P1) |
| Label styling: font, size, background, connector, outline | `[x]` | `[x]` | `[ ]` (P1) |
| Distance / angle / dihedral objects with dashes | `[x]` | `[x]` | `[~]` typed spec and commands; dash and arc styling open (P1) |
| Unit cell, map extent | `[x]` | `[x]` | `[~]` public unit-cell guides and inverse exist; triclinic native smoke performed, map extent and cross-engine comparison remain open (P2) |
| Symmetry mates / assemblies / supercell | `[x]` | `[x]` | `[ ]` placement lowering and instanced bonds open (P0) |
| Isosurface / mesh / dot of density | `[x]` | `[x]` | `[~]` volume pass, no mesh or dot style (P1) |
| Direct volume with transfer function | `[x]` | `[x]` | `[~]` (P1) |
| Slice through a map | `[x]` | `[x]` | `[ ]` (P2) |
| Segmentation | `[x]` | `[ ]` | `[x]` |
| Trajectory playback and interpolation | `[x]` | `[x]` | `[x]` |
| Movies: keyframe views, scenes, frame export | `[x]` | `[x]` | `[ ]` (P2) |
| Alignment objects | `[ ]` | `[x]` | `[ ]` (P3) |

### Colour and size

| Capability | Mol* | PyMOL | MolGFX |
|---|---|---|---|
| Element / CPK | `[x]` | `[x]` | `[x]` |
| Chain / entity / polymer / unit / operator | 8 themes | `[x]` | `[~]` chain and entity; polymer, unit and operator need assemblies (P1) |
| Residue name, residue charge | `[x]` | `[x]` | `[x]` residue name by chemistry; residue charge via `formal_charge` |
| Secondary structure | 11 states | 3 | `[~]` 5 states; the rest needs DSSP's full alphabet in MolFrame (P1) |
| Molecule type | `[x]` | `[ ]` | `[x]` |
| Carbon-by-chain (`cbc`, `cbag`, `cba*`, `cnc`) | `[x]` | `[x]` | `[x]` `carbon_by_chain`; per-colour `cba*` via a uniform rule |
| Sequence rainbow / `spectrum` | turbo, 28 stops | 29-stop `rainbow`, 60 palettes | `[x]` `sequence_position` on `rainbow` / `turbo` |
| Occupancy, B-factor / uncertainty, charge, hydropathy | `[x]` | `[x]` | `[x]` as `AtomMetric` |
| Ramp catalogue | ColorBrewer + turbo | 60 named + `ramp_new` | `[~]` 30 ramps × reversal, up to 16 stops; PyMOL's remaining named palettes (P2) |
| Colour by SASA, by electrostatics, by distance to volume | `[x]` | `[x]` | `[~]` SASA is a metric scheme; electrostatics and distance-to-volume are open (P2) |
| Per-selection colour rules with priority | `[x]` | `[x]` | `[x]` |
| Size themes (physical, uniform, uncertainty, volume-value) | `[x]` | `[x]` | `[~]` physical and uniform (P1) |
| Per-element transparency, emissive, overpaint, substance, wiggle | `[x]` | transparency only | `[ ]` (P1) |
| Named colour table | 1500+ | 196 | `[~]` hex plus 42 named colours; the full reference palettes remain open (P2) |
| Colour-blind safe categorical set | partial | none | `[x]` categorical palette is CVD-safe |

### Rendering and image quality

| Capability | Mol* | PyMOL | MolGFX |
|---|---|---|---|
| Analytic impostors, deferred lighting | `[x]` | `[x]` | `[x]` |
| Ambient occlusion | multi-scale SSAO | `[x]` | `[~]` single scale (P1) |
| Shadows | `[x]` | ray | `[x]` |
| Outline (depth edge) | `[x]` | ray modes 1–3 | `[x]` illustration silhouette and cavity |
| Marker edge and ghost | `[x]` | overlays | `[~]` edge yes, ghost no (P0) |
| Fog / depth cue | `[x]` | `[x]` | `[x]` |
| Transparency | WBOIT, DPOIT | multilayer, OIT | `[~]` WBOIT (P2 DPOIT) |
| Anti-aliasing | SMAA, FXAA, TAA | FXAA, SMAA, ray AA | `[~]` FXAA + TAA; SMAA open (P1) |
| DOF, bloom, motion blur | DOF, bloom | none | `[x]` |
| Contrast-adaptive sharpening | `[x]` | `[ ]` | `[ ]` (P3) |
| Cel shading, x-ray shading | `[x]` | `[ ]` | `[~]` posterize; no x-ray (P2) |
| Materials (metalness, roughness, bump) per representation | `[x]` | specular / shininess | `[~]` material profiles, no bump (P2) |
| Interior colour on clipped surfaces | `[x]` | `[x]` | `[ ]` (P2) |
| Clip objects: plane, sphere, box, cylinder, cone | `[x]` | slab | `[~]` planes and caps (P1) |
| Backgrounds: gradient, image, skybox | `[x]` | gradient | `[~]` gradient (P2) |
| Path-traced global illumination | `[x]` | ray tracer | `[ ]` (P3) |
| Stereo, XR | `[x]` | 10 modes | `[ ]` (P3) |
| Sphere LOD, Hi-Z occlusion | `[x]` | none | `[ ]` tile-binned LOD only (P1) |
| Orthographic camera, field of view | `[x]` | `[x]` | `[x]` |

### Interaction, selection and export

| Capability | Mol* | PyMOL | MolGFX |
|---|---|---|---|
| Selection language | MolQL + transpilers | native | `[x]` MolFrame queries |
| Granularity: atom / residue / chain / instances | 7 | 4+ | `[~]` atom + residue (P1) |
| Hover / click / marker | `[x]` | `[x]` | `[x]` |
| Focus and context, orient-to-axes, snapshots | `[x]` | `[x]` | `[~]` focus; orient and snapshots open (P1) |
| Representation presets | 11 | classified defaults | `[~]` size preset (P1) |
| Image export | png / jpeg / webp | png / ray | [~] PNG and Rust HDR/OpenEXR; JPEG/WebP open |
| Geometry export | glb, obj, stl, usdz | obj, stl, dae, wrl, idtf, pov | [ ] (P2) |
| Frame sequences / movie encoding | image sequences / mp4 | mpng + encode | [~] bounded converged sequences; MP4/GIF encoding is external |

## Performance

Nothing in this repository supports a claim that MolGFX is faster than Mol* or
PyMOL. The table below is historical MolGFX-only evidence from
`cargo run --release -p molgfx-bench --bin frame_time STRUCTURE FORM`,
Apple GPU through Metal, 1280×720, structure 1AON (58 870 atoms), default
adaptive profile (`Standard` at that size). It is not a current maximum-quality
comparison. Its device timestamps did not resolve; durations are blocking
end-to-end wall time.

| Form | Frame | Reading |
|---|---|---|
| Cartoon | 3.4 ms | fine |
| Spacefill | 9.1 ms | acceptable, but a 60 fps budget is 16.7 ms and shadows + AO share it |
| Licorice over all atoms | 31.6 ms | too slow: about 32 fps |
| Solvent-excluded surface of the whole complex | 138 ms | too slow: about 7 fps |

- **Open, P0:** find where licorice and surface spend their time at the real
  comparison settings. Actual per-pass capture now exists, including bounded
  coverage and timestamp absence reasons; that is not yet attribution for
  these historical runs.
- **Open, P0:** the same four cases in Mol* (browser, `pickScale` default) and
  PyMOL (`png` without ray) on identical input and camera, before any
  comparative statement is made. Time to first frame and steady frame time are
  the two numbers.
- **Open, P1:** memory per atom, upload volume per edit, and wasm cold start.
- Publication rendering accumulates 64 samples and is not a frame rate.

### Measured baseline — 2026-10-01, Apple M5 Pro, Metal, 1280×720

`frame_time` on 7qpd (11,521 atoms), HighestFixed, one completed output = 64
converged samples, 10 warm-up and 60 measured outputs, serial. These are the
first numbers recorded under the corrected completion contract; they replace the
historical table above for any comparison.

| Form | Completed outputs/s | Median per output | Per sample | CPU median | Reading |
|---|---:|---:|---:|---:|---|
| Cartoon | 16.8 | 59 ms | 0.93 ms | 11.7 ms | the 120 outputs/s target needs 0.13 ms per sample: 7× short |
| Spacefill | 0.8 | 1263 ms | 19.7 ms | 15.3 ms | traced AO is ≈ 18.4 ms of every sample |
| Surface | refused | — | — | — | `surface_spacing_effective` is 0.685 Å against 0.25 Å requested, so the output is correctly **not complete**; needs bricked fields |

Attribution (per-pass actual timestamps, spacefill): sphere impostors 0.7 ms,
scene-fit shadows 0.3 ms, traced ambient occlusion ≈ 18.4 ms. Setting the
publication AO budget from 8 to 2 rays per sample gave 359 ms per output
(3.5× faster), so the cost is linear in rays and the BVH traversal is not the
bottleneck: 8 rays × 64 samples is 512 AO rays per pixel per output. Whether
that budget can fall at equal converged error is a measurement still to be
made (render N=8 and N=2 against a 1024-sample reference and compare error);
it is not assumed here.

Consequences for the stated goal: a 64-sample converged output at 120 per second
is not reachable on this hardware by tuning. Interactive mode (one sample per
frame with temporal accumulation) is a separate recipe and has its own number.

### Shared-corpus harness evidence

The registered `parity` CLI verifies licensed input hashes before engine startup,
inspects metadata before producing images, and retains per-recipe failures.
Native cold, warm-up and measured outputs carry CPU stage intervals, actual
per-pass device timings/reasons, capture coverage, allocator measurements and
residency counters. The shared nearest-rank summarizer excludes cold/warm rows.
External counters that are not instrumented are null with reasons, never measured
zeros; external `cpu_ns` is completion-inclusive API wall time, not exclusive CPU.

An actual 13-fixture run at 128×72, one cold output, one warm-up and one measured
output is recorded in `/tmp/molgfx-parity-telemetry-full13/report.json`. Eleven
native molecular cases completed all 64 publication samples and their PNGs
decoded at the requested extent. Captures retained all 640 or 704 actual passes
without overflow; several occurrences had `invalid_order` timestamps, while
the measured lattice-8 output resolved all 640. Native density cases remain
explicit prerequisites: volume-only scenes and an affine VolumeBinding, not
a fake molecular anchor or flattened skew. The smoke is not a 120-FPS or
maximum-resolution performance certification.

Protein inspection is retained in
`/tmp/molgfx-parity-telemetry-metadata/4HHB/metadata.json`: both engines saw
4,779 atoms, but native/PyMOL counts were 801/584 residues, 4,475/4,700 bonds
and 479/0 aromatic bonds; secondary-structure and provenance differences remain
visible rather than reassigned to manufacture parity. The initial PyMOL raster
boundary failed under headless `-cq`: `cmd.png(prior=1)` reported `no prior image
available` after `cmd.draw` (retained in `/tmp/molgfx-parity-telemetry-pymol13`).
The corrected invocation uses real `-q` OpenGL initialization for raster only,
while inspection and ray remain headless. Its registered CLI run at the same
128×72 extent completed all 13 fixtures, including both densities:
`/tmp/molgfx-parity-telemetry-pymol-gl13/report.json`. All 13 PNGs decoded at
the requested extent; actual cold/warm/measured wall-time rows and null+reason
unmeasured-counter fields were verified at that image boundary. The manifest
raster preset used antialias=0; these are execution-contract smoke results,
not maximum-quality equivalence or comparative speedup evidence.

## Completed baseline contracts

- [x] One browser runtime for Jupyter and the docs site.
- [x] Coordinates remain borrowed through the MolFrame seam; no scene copy was
  introduced for rendering.
- [x] Shared GPU record sets, indirect draws, lazy residency, and capability-
  driven occupancy formats remain the performance architecture.
- [x] No product `unsafe`, avoidable `unwrap`, or parallel per-atom draw path
  was added by this work.

## Current working-tree handoff

The current pass closes only the following boundaries; the open items above
remain intentionally open for the next contributor:

- `DepthCue` is now a validated public profile input and a frame/deferred
  shader input. It has Rust/API coverage but no dedicated browser fog fixture.
- Anti-aliasing is a first-class presentation module: `RenderProfile` carries
  an `AntiAliasing` style, `with_edge_smoothing` sets it in Rust and Python, and
  the tonemap pass reads the resolved choice. Unset keeps the previous
  tier default (realtime smooths, converged does not).
- The frame-sequence export (`Renderer::render_sequence`, Python
  `render_sequence(scene, size=..., fps=..., frames=...)`) streams a bounded
  number of converged frames and returns them in submission order; movie
  *encoding* stays the caller's job.
- Colour by SASA is a metric scheme (`AtomMetric::Sasa`) computed with
  Shrake–Rupley on the live coordinates; the named colour table is 42 names.
- Adaptive quality has atom-count caps; HighestFixed explicitly bypasses them.
  Both consume the shared detail policy. Upload budget changes must preserve
  live tickets/fences; full-residency certification remains open as noted above.
- Shared interaction WGSL owns hidden-state visibility and state tinting.
  Marker-specific occluded edge ghosting is not implemented: it needs a mask of
  fragments that lost the depth test, which is a second draw of marked geometry
  without depth test, not a change to the existing payload path.
- Every analytic depth writer now ranks a coincident fragment by entity
  (`stable_entity_depth`): the molecular capsule and wire paths gained it, and
  the two provider-backed paths rank by pick page and local row. Two identical
  renders of a licorice fixture are byte-identical. The point pipeline is exempt
  because it writes the rasterized device depth. **Check:**
  `every_analytic_depth_writer_ranks_a_coincident_tie_by_entity`.
- Browser hover is latest-only and invalidated on camera/scene changes, but
  still performs per-request readback rather than using a cached low-resolution
  buffer.
- Multi-bond order, aromatic inner strokes and the reference 0.25 Å stick
  radius are now drawn, and the Lines lone-atom cross is implemented end to
  end; licorice's exact junction cap and half-bond seam are still open.
- The `molgfx-api` → `molgfx-scene` crate rename and the `science` → `overlay`
  module rename are complete across crates, Python, WASM, the docs site and the
  guides; no `scien`-named engine concept remains outside the PyPI trove
  classifiers, which classify the package rather than name the engine.

The exact changed files, observed test counts, and recommended implementation
order are recorded in the continuation checkpoint at the end of `mapping.md`.
Do not promote `[~]` or `[ ]` entries to `[x]` without the named fixture,
measurement, or serialization boundary.
