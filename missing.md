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
  index, and `scene/apply.rs` lowers `AssemblySpec.instances` to placement
  metadata only. Adding it needs an `InstancedBond` row and an instance lane on
  the packed bond record, both on the MolGFX side.

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
- [~] **P0** Lines use screen-space quads and render topology bonds. **Open:**
  lone-atom cross markers, line anti-alias policy, and reference width parity.
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
  - **Plane** and **unit cell** are guide geometry with a different lifecycle
    from a representation: core lowers them through `Scene::add_planar_region`
    and `Scene::add_unit_cell` into the guide table, but no engine-crate,
    facade, command or Python path reaches those core methods. Making them
    public means a guide-based scene item (patch operation, lowering and
    resolved-handle plumbing), not a representation form.
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
  upload budget is fixed at engine construction (`ResidencyConfig`), so a tier
  cannot yet scale it; that needs a rebuildable staging configuration, not
  another tier field, and a full Mol* policy comparison needs the reference
  render.
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
  streams bounded frames and returns them in submission order. **Open:** movie
  *encoding* (deliberately the caller's job), JPEG/WebP output, and Mol* `ImagePass`
  / PyMOL ray-setting parity. **Check:** an observed Python run returning three
  64×48 PNG frames.

## Comparison and verification backlog

- [x] **P1** Temporary three-engine harness created and exercised common atom,
  bond, cartoon, line, surface, label, point, and quality cases. PyMOL and
  MolGFX used the same temporary PDB; Mol* browser cases used the checked-in
  browser fixtures. Results and blocked network/native paths are recorded in
  `mapping.md`.
- [~] **P0** Same-input image comparison for protein, nucleic-acid, ligand, and
  density fixtures. **Open:** add the fixtures and explicit camera/theme/form
  settings to a reproducible comparison command; do not use raw pixel equality
  across engines with different defaults.
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
| Orientation glyph, polyhedron, plane | `[x]` | `[ ]` | `[ ]` orientation, polyhedron and plane: core has the guide primitives (`add_planar_region`, `add_unit_cell`) but no engine-crate, facade or Python path reaches them (P3) |
| SNFG carbohydrate symbols and links | `[x]` | `[ ]` | `[~]` glycan ribbon, no SNFG glyphs (P2) |
| Residue beads / coarse | `[x]` | `[ ]` | `[x]` new |
| Atom labels (text on atoms, by property) | `[x]` | `[x]` | `[~]` free labels only (P1) |
| Label styling: font, size, background, connector, outline | `[x]` | `[x]` | `[ ]` (P1) |
| Distance / angle / dihedral objects with dashes | `[x]` | `[x]` | `[~]` typed spec and commands; dash and arc styling open (P1) |
| Unit cell, map extent | `[x]` | `[x]` | `[~]` core guide exists, no public form (P2) |
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
| Image export | png / jpeg / webp | png / ray | `[x]` png (P1 other formats) |
| Geometry export | glb, obj, stl, usdz | obj, stl, dae, wrl, idtf, pov | `[ ]` (P2) |
| Movie export | mp4 | mpng + encode | `[ ]` (P2) |

## Performance

Nothing in this repository supports a claim that MolGFX is faster than Mol* or
PyMOL. The numbers below are MolGFX alone, measured with
`cargo run --release -p molgfx-bench --bin frame_time STRUCTURE FORM` on an
Apple GPU through Metal, 1280×720, structure 1AON (58 870 atoms), default
adaptive profile, which holds the `Standard` tier at that size. The frame time
is the blocking end-to-end duration of one frame; device timestamps do not
resolve on this adapter.

| Form | Frame | Reading |
|---|---|---|
| Cartoon | 3.4 ms | fine |
| Spacefill | 9.1 ms | acceptable, but a 60 fps budget is 16.7 ms and shadows + AO share it |
| Licorice over all atoms | 31.6 ms | too slow: about 32 fps |
| Solvent-excluded surface of the whole complex | 138 ms | too slow: about 7 fps |

- **Open, P0:** find where licorice and surface spend their time. There is no
  per-pass GPU breakdown yet (the pass graph carries timestamp slots but no
  reporter), so attribution is a guess until one exists. Add it first.
- **Open, P0:** the same four cases in Mol* (browser, `pickScale` default) and
  PyMOL (`png` without ray) on identical input and camera, before any
  comparative statement is made. Time to first frame and steady frame time are
  the two numbers.
- **Open, P1:** memory per atom, upload volume per edit, and wasm cold start.
- Publication rendering accumulates 64 samples and is not a frame rate.

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
- Adaptive quality has an atom-count cap at 10,000 / 100,000 / 500,000
  atoms and drives surface spacing and ribbon samples through one `TierDetail`.
  It does not yet control the upload budget, which is fixed at engine
  construction in `ResidencyConfig`; scaling it per tier means a rebuildable
  staging configuration, not another tier field.
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
  radius are now drawn; licorice's exact junction cap and half-bond seam and the
  lone-atom cross for `lines` are not yet implemented.
- The `molgfx-api` → `molgfx-scene` crate rename and the `science` → `overlay`
  module rename are complete across crates, Python, WASM, the docs site and the
  guides; no `scien`-named engine concept remains outside the PyPI trove
  classifiers, which classify the package rather than name the engine.

The exact changed files, observed test counts, and recommended implementation
order are recorded in the continuation checkpoint at the end of `mapping.md`.
Do not promote `[~]` or `[ ]` entries to `[x]` without the named fixture,
measurement, or serialization boundary.
