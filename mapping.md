# Engine map: Mol* and PyMOL → MolGFX

This is the source-and-render map for the engine layer. It compares the
checked-out Mol* 5.11.0 and Open-Source PyMOL trees with the current MolGFX
workspace. Viewer/plugin state trees, sequence panels, docking workflows, and
movie editors remain application responsibilities; MolGFX exposes the scene,
render, query, and interaction contracts those applications need.

Status words:

- **have** — the capability exists in the checked-in API and render path.
- **partial** — the main path exists, but parity, input coverage, or a quality
  contract is still narrower.
- **missing** — no equivalent engine capability is implemented.
- **unverified** — the code path exists, but no comparable fixture/render has
  been exercised yet.
- **external** — supplied by MolFrame or a caller rather than implemented in
  MolGFX.

## Evidence and comparison harness

The source audit used these authoritative reference areas:

| Engine | Source areas |
|---|---|
| Mol* | `../molstar/src/mol-repr`, `mol-theme`, `mol-model`, `mol-script`, `mol-gl`, `mol-canvas3d`, `mol-plugin`; representation registries are in `src/mol-repr/structure/registry.ts` and `src/mol-repr/volume/registry.ts`. |
| PyMOL | `../pymol-open-source/layer1/Rep.h`, `SettingInfo.h`, `SceneRender.*`, `ScenePicking.*`; `layer2/Rep*.cpp`, `CoordSet.cpp`, `AtomInfo.h`; `layer3/Selector.cpp`; setting descriptions in `data/setting_help.csv`. |
| MolGFX | `crates/molgfx-scene`, `crates/molgfx-command`, `crates/molgfx-core`, `crates/molgfx-geometry`, `crates/molgfx-render`, `crates/molgfx-shaders`, and `crates/molgfx-wasm/js`. |

A temporary cross-engine fixture was rendered during this review under
`/tmp/molgfx-3engine-comparison`:

- PyMOL 3.1.0 rendered ball-and-stick, cartoon, lines, and surface/labels with
  `nix shell nixpkgs#pymol --command pymol -cq <case>.pml` and `png`. The
  cartoon script emitted an unknown-colour diagnostic for `chainbow`; its image
  is retained as exploratory evidence, not a clean parity result.
- MolGFX rendered all currently command-addressable atom, bond, polymer, and
  surface forms through the generated browser runtime. The 12 form cases
  completed with no browser page errors. A separate `nucleic.pdb` browser case
  also reached `nucleic.pdb ready.` with no page or console errors; its image
  and matching PyMOL render are retained beside the main fixture.
- Mol* browser tests were built with `npm run dev:browser-tests`; the
  `render-structure`, `render-spheres`, `render-lines`, `render-mesh`, and
  `render-shape` cases rendered with no browser errors. The structure case
  loads Mol*'s `3pqr` fixture and exercises cartoon, interaction, atomic, and
  membrane representations.
- Mol*'s Node `mvs-render` route is a real headless image path
  (`src/cli/mvs/mvs-render.ts`), but its optional native `gl`/`canvas` modules
  did not install for Node 24 on this arm64 host. The browser route was used
  instead; this is an environment limitation, not a Mol* feature claim.
- The Mol* structure-grid browser case was not counted as green: its four
  PubChem requests were blocked by the browser network allowlist.

These images are visual evidence, not golden parity tests. The engines choose
 different defaults, camera fitting, backgrounds, and lighting; pixel equality
would be a false acceptance criterion. The useful comparison is whether the
same structural input, form, query, theme, and interaction semantics produce the
same *kind* of scene.

## 1. Topology and source perception

| Capability | Mol* | PyMOL | MolGFX status and source |
|---|---|---|---|
| File bonds | `chem_comp_bond`, `struct_conn`, SDF/MOL2 pairs, and computed-bond coverage policy in the structure model | CONECT, component connectivity, and `connect_mode` policies | **have** for MolFrame PDB/CIF reads. File bonds remain first in `molframe-chem/src/bonds.rs`; inferred bonds are distinguishable and CIF writing omits them. |
| Standard residue chemistry | Component tables and atomic bond properties | `ConnectComponent` tables plus polymer-link rules | **have** in `../molframe/crates/molframe-chem/src/standard_bonds.rs`: amino acids, nucleotides, water, aromatic rings, and known double bonds. |
| Polymer links | Structure model inter-unit links | C–N and nucleic backbone connectivity | **have**: C–N and O3′–P links are added by the MolFrame enrichment pass. |
| Distance fallback | Spatial cell/grid candidates and element thresholds | Grid-backed distance/connect modes | **have**: sorted spatial cells, 27-neighbour traversal, element thresholds, H–H exclusion, alternate-location compatibility, and metal coordination metadata in `molframe-chem/src/bonds.rs`. |
| Bond order, aromaticity, metal coordination | Bond order/type fields and flags | Bond order 0–4, valence, coordination behavior | **have in the data and shader seams**: `SourceBond` carries `BondOrder`, aromatic, and metal flags; `BondGpu` carries order and flags; the bond impostor shader emits aromatic/metal dash cadence. Visual golden coverage remains open. |
| Rings and aromaticity | Ring/planarity-aware bond and query machinery | `byring` and valence/ring heuristics | **have** for the chemistry table and aromatic metadata; broad ring perception remains MolFrame-owned. |
| File secondary structure | `struct_conf` and `struct_sheet_range` | HELIX/SHEET and mmCIF secondary categories | **have**: PDB and CIF readers parse records into MolFrame secondary-structure rows. |
| DSSP-like assignment | H-bond/geometry-based secondary structure | `dss` and automatic one-state assignment | **have in MolFrame**: grid-backed candidate search and H-bond classification in `molframe-chem/src/secondary.rs`; the facade enriches read inputs. |
| CA-only fallback | Zhang–Skolnick-style geometry fallback | No equivalent required | **have in MolFrame** and used when file/DSSP rows remain unknown. |
| Automatic precedence | File annotation, computed provider, fallback | File data then automatic DSSP policy | **have**: `molframe::read_buffer` applies file → perception → fallback, while caller-created snapshots opt into the same contract through `molframe::perceive` without copying coordinates. |
| Trace/direction atoms | CA/O3′ and polymer-specific guide atoms | CA/P and nucleic guide atoms | **partial**: MolGFX cartoon/trace paths consume the MolFrame hierarchy; fixture-level nucleic and branched-polymer comparison remains unverified. |
| Symmetry mate/inter-unit bonds | Unit/symmetry instance links | Symmetry-operation bonds | **missing** in the MolGFX renderer path; instancing exists, but source-bond generation across symmetry mates is not supplied. |
| Components and classifications | Polymer, ligand, carbohydrate, solvent, ion, membrane, and volume components | Polymer/organic/inorganic/solvent classes | **partial**: MolFrame queries expose residue/entity data; a complete preset classifier is not yet authoritative in MolGFX. |
| Altloc and hydrogen policies | Explicit altloc and `ignoreHydrogens` parameters | Alternate-location and hydrogen settings | **partial**: bond inference respects compatible altlocs and the source fields are preserved; a single public atom-form policy is still missing. |

## 2. Representation forms

MolGFX has one typed form registry in
`crates/molgfx-scene/src/representation/form.rs` and one command registry in
`crates/molgfx-command/src/ir/form.rs`. The forms below are not separate
renderer implementations: they lower into shared analytic/impostor, ribbon,
surface, point, label, volume, and interaction passes.

| Form | Mol* | PyMOL | MolGFX status |
|---|---|---|---|
| Spacefill / spheres | `spacefill`, physical or uniform size, sphere impostors | `sphere` with vdW scale and sphere modes | **have**; rendered in the temporary browser case. |
| Ball-and-stick | element spheres plus intra/inter-unit bonds and half-bond colors | spheres plus sticks, valence, and bond order | **have**: atoms and file/inferred bonds render; order 2 and 3 draw as parallel strands, aromatic as a solid line with a broken inner line, and order 0 dotted. Exact half-bond colours and the metal cadence are not pixel-compared. |
| Licorice / sticks | bond/atom cylinder variants | `sticks`, junction spheres, half-bond colors | **partial**: native bond-capable form renders at the reference 0.25 Å radius; exact junction cap and half-bond seam are not implemented. |
| Lines | line bonds, points/crosses for lone atoms | `lines`, `nonbonded`, smooth line modes | **partial**: screen-space bond wires and atom points render; lone-atom cross and all width/anti-alias policy parity is not complete. |
| Points | point/element visuals | point/sprite sphere modes | **have**; public `Points` form. |
| Dots | volume dots and dot surfaces | solvent/van der Waals dot density | **partial**: public analytic dot form exists; complete vdW/SAS buried-dot behavior is not equivalent. |
| Cartoon | polymer curve, helix/sheet/coil, nucleotide blocks/rings, gaps, arrows | cartoon, putty, nucleic options, smoothing and transparency | **partial**: quality-scaled spline/ribbon and secondary-structure inputs exist; full Mol*/PyMOL sheet-arrow, nucleotide, and fixture coverage remains. |
| Backbone | cylinder/sphere/gap guide representation | usually a trace/ribbon variant | **have** as a typed form and render route. |
| Trace | thin polymer guide curve | CA/trace guide lines | **have** as a typed form and render route. |
| Tube | round smooth guide tube | ribbon/trace-like guide geometry | **have** as a typed form and render route. |
| Putty | uncertainty/B-factor-sized polymer tube | `cartoon putty` | **have** as a typed form; B-factor domain/radius controls lower through the shared ribbon path. |
| Molecular surface | molecular surface mesh/wireframe | `surface`, carving, solvent radius, mesh | **have** for the four MolGFX surface kinds and styles; temporary solvent-excluded case rendered. Resolution and carving parity remain partial. |
| Gaussian/blob surface | Gaussian surface/volume and blob surface | no direct Gaussian equivalent | **partial**: the Gaussian kind and the soft-union (blob) style are both reachable (`rep.surface(kind=..., style='soft_union')`, command and Python); a dedicated blob *form* is not separate. |
| Nucleic acid / bases / base pairs | nucleotide blocks, rings, bonds, elements | cartoon nucleic options and ladders | **partial**: `NucleicAcid`, `Bases`, and `BasePairs` are public; a shared nucleic fixture has not yet been rendered through all three engines. |
| Glycan | carbohydrate symbols and links | no standard equivalent | **partial**: `Glycan`/Twister/PaperChain paths exist; SNFG shape/color fixture coverage remains. |
| Labels and annotations | SDF label text and loci labels | FreeType labels, connectors, screen/world placement | **partial**: deterministic label/guide/marker passes and API specs exist; no command form exposes all label controls. |
| Measurements | distance, angle, dihedral loci/visuals | distance/angle/dihedral measurement objects | **partial**: typed measurement specs and interaction glyphs exist; command and cross-engine fixture coverage is incomplete. |
| Volumes and segmentation | direct volume, isosurface, segment, slice, dot | map surface/mesh/volume | **partial**: MolGFX volume/segmentation contracts and passes exist; no comparable density/map fixture was included in the smoke set. |
| Ellipsoid/orientation/polyhedron/plane/unit cell | ADP ellipsoids, orientation, coordination polyhedra, plane and cell helpers | ellipsoids and cell/measurement features | **partial/missing by form**: low-level primitives exist for some paths, but no complete curated public form/command parity. |

## 3. Themes, color, and size

| Theme or policy | Mol* | PyMOL | MolGFX status |
|---|---|---|---|
| Element/CPK | `element-symbol`, element index, illustrative variants | `atomic`, object/element palettes | **have**: element color registry and shader path. |
| Chain/entity/polymer | chain/entity/source themes and legends | `chainbow`, object/chain cycles | **have** for chain and related semantic colors; palette parity is not promised. |
| Residue/secondary structure | residue, cartoon, secondary-structure themes | `resn`, `cbss`, cartoon defaults | **have** as API values; useful SS colors require the enriched SS column. |
| Uniform/hex colors | arbitrary color and opacity | named/hex colors and transparency | **have** through `ColorSpec`, command `color`, and `opacity`. |
| Scalar ramps | uncertainty, occupancy, volume, hydrophobicity, charge, property legends | `spectrum b`, charge and property settings | **partial**: bound scalar-property/ramp color specs exist; occupancy, hydrophobicity, molecule-type, and all built-in Mol* palettes are not exposed. |
| Carbon-by-chain element behavior | Mol* special carbon-by-chain default | PyMOL object/chain color defaults | **missing as a single default policy**: MolGFX's element scheme keeps carbon neutral unless the caller chooses chain color. |
| Sequence rainbow | spectrum/turbo style palettes | spectrum rainbow | **missing** as a named command/theme. |
| Physical atom/bond size | vdW, coarse, endpoint-dependent bond size | vdW and stick/sphere settings | **have** for physical/uniform form parameters; exact default constants differ. |
| Uncertainty size | uncertainty/putty | putty and B-factor settings | **partial**: Putty is available; uncertainty is not a universal size theme. |

## 4. Marking, selection, and picking

| Capability | Mol* | PyMOL | MolGFX status |
|---|---|---|---|
| Dense per-element state | marker textures and loci actions | per-atom selection membership | **have**: selected, hovered, focused, muted, hidden, and custom interaction channels resolve to dense state words in `scene/interaction.rs`. |
| Default visual tint | highlight/select marker colors with priority | selection indicators/overlays | **have** for built-in atom/bond/visual shader paths; selected/hovered/focused/muted state is consumed by shared WGSL. |
| Edge outline/ghosting | marking mask, edge postprocess, ghosted occlusion | ray/selection overlays but no same Mol* pass | **partial**: illustration outline/depth-cue profile controls exist; a marker-specific mask-to-edge pass with ghosted hidden edges is not complete. |
| Rich atom pick | Loci resolves model/entity/chain/residue/atom and labels | object path plus AtomInfo fields | **have** in `molgfx-scene::scene::inspect`: stable source IDs, label/auth names, altloc, element, occupancy, B-factor, charge, position, and SS; WASM serializes the same `ResolvedPick`. |
| Bond pick | bond loci and bond locations | bond/atom pick identifiers | **have**: static and dynamic bond picks resolve to source endpoints, topology revision, aromatic/metal flags, static order, and dynamic weight through `ResolvedBondPick`. |
| Picking buffer | cached scaled viewport and async readback | multipass color-index/15×15 pick window | **partial**: MolGFX has packed pick tokens and async per-request readback, and now resolves label, measurement and volume-segment picks to their scene item; cached low-resolution hover buffers are not implemented. |
| Click selection | loci granularity, toggle/union/remove, empty clear | configurable mouse selection mode, commonly residue selection | **partial**: browser local-file mode applies replace/Shift-add/Alt-remove/Escape-clear patches and publishes a typed event; the local query now selects the picked residue while preserving the atom pick report, and remote hosts own their mutation. |
| Hover | throttled latest-only highlight | no equivalent default | **partial**: latest-only async hover and local hovered channel work; it is not a cached low-resolution buffer. |
| Pick report | human-readable loci label plus source IDs | PyMOL path/AtomInfo | **have**: atom and bond records plus `label`, `measurement` and `volume_segment` variants with their own fields, all round-tripping through serde. |

## 5. Rendering and image quality

| Feature | Mol* | PyMOL | MolGFX status |
|---|---|---|---|
| Analytic impostors | sphere/cylinder/line primitives with fragment depth | sphere/stick shaders and ray renderer | **have**: sphere, bond capsule, line, point, and analytic surface paths. |
| Lighting/materials | GGX/head light/ambient, illustrative variants | two-light and ray lighting settings | **have**: material/BRDF/lighting profiles; constants are intentionally not identical. |
| Ambient occlusion | multiscale SSAO and denoise | surface/ray shading | **partial**: AO and denoise passes exist; Mol* multiscale threshold parity is not yet matched. |
| Shadows | screen-space/soft options | ray shadows | **have** in the render graph, with profile-dependent quality. |
| Transparency | WBOIT/DPOIT/blended modes | sorted/WBOIT/ray transparency | **have** through OIT paths. |
| Temporal resolve | jitter, accumulation, marker stability | no equivalent realtime temporal contract | **have in graph; unverified** for convergence/marker-only changes. |
| Anti-aliasing | SMAA/FXAA and multisampling | MSAA/line smoothing/ray AA | **partial**: temporal resolve plus FXAA, now selectable through `RenderProfile.with_edge_smoothing` (Rust and Python) with a tier default; SMAA is open. |
| Outline/depth cue/fog | outlines, background/depth cue, DOF, bloom | ray modes and fog/depth cue settings | **partial**: illustration silhouette/cavity controls and an explicit `DepthCue` profile input now lower through the frame uniform and deferred path; marker-specific mask/edge ghosting and cross-engine fog parity remain open. |
| DOF/bloom/motion blur | postprocessing passes | ray/scene effects vary by mode | **have** as explicit optional MolGFX profile effects. |
| Camera fitting | perspective/orthographic, fitted clip planes | perspective/orthoscopic and orient | **have**: camera API and fit path. |
| Image export | offscreen `ImagePass`, PNG/JPEG/MP4 helpers | PNG/ray and movie export | **partial**: deterministic PNG plus a bounded streaming frame sequence (`render_sequence`) exist; JPEG/WebP and movie encoding are open. |

## 6. Scale and performance

| Technique | Mol* | PyMOL | MolGFX status |
|---|---|---|---|
| Instancing | unit/symmetry instances and grouped renderables | object/state reuse | **have/partial**: shared scene records, symmetry instance primitives, lazy residency, and indirect draws exist; symmetry-bond generation is open. |
| Culling | instance grid, frustum/occlusion culling, LOD and multi-draw | display/object culling and quality settings | **have**: GPU culling/indirect draw and chunk residency; Hi-Z/sphere-stride parity remains open. |
| Quality policy | atom-count/resolution-dependent geometry quality | atom-count/cartoon quality settings | **partial**: adaptive quality now applies one authoritative atom-count cap at 10k/100k/500k bands and remains monotone under scene shrink; geometry segmentation, surface resolution, upload budgets, and full Mol* threshold parity remain open. |
| GPU memory | grouped buffers and reusable renderables | CPU/OpenGL display lists/buffers | **have** for columnar/lazy upload design; actual large-scene benchmark coverage is open. |
| Idle work | settled render state avoids unnecessary updates | scene invalidation/display rebuilds | **have** in the render graph and residency design. |
| Marker updates | texture/state update without geometry rebuild | selection/display invalidation | **have** in the semantic state path; outline consumer remains open. |

## 7. Queries and presets

| Capability | Mol* | PyMOL | MolGFX status |
|---|---|---|---|
| Selection language | MolScript/MolQL plus PyMOL/VMD/Jmol transpilers | rich selection operators (`byres`, `within`, `around`, `expand`, `bychain`, …) | **have** for MolFrame query syntax; exact PyMOL keyword coverage is not claimed. |
| Named selections/layers | state tree and loci selections | named selections/objects | **have** for command named selections/layers in the facade; host synchronization owns external state. |
| Focus/context | loci focus and camera/selection behaviors | zoom/orient and selection commands | **have**: typed focus interaction with a 6 Å residue context. |
| Size-based default preset | coarse/large/huge presets | `auto_show_classified` | **have**: `Scene::add_auto`, the `auto` command and `Scene.auto()` classify by size and chemistry; solvent is never drawn. |
| Assembly/fitting/validation/movie/snapshot commands | application/plugin workflows | command/session workflows | **partial/out of engine command surface**: some typed command IR exists, but the text registry deliberately exposes only the current scene-authoring subset. |

## 8. Concrete parity risks to fix

1. **Visual bond semantics:** done for order 2/3, aromatic and order 0; the
   metal cadence and exact half-bond colours are the remainder.
2. **Selection granularity:** make browser click selection residue-aware while
   retaining the rich atom pick record.
3. **Themes:** add named occupancy, hydrophobicity, uncertainty, molecule-type,
   and sequence-rainbow inputs through the single MolGFX input registry.
4. **Marking:** finish marker-specific edge/ghost rendering and cached hover
   picking without rebuilding geometry.
5. **Quality:** settle the size-based quality thresholds and verify temporal,
   AO, AA, and large-scene behavior with measurements rather than screenshots
   alone.
6. **Coverage:** add shared protein, nucleic-acid, ligand, and density fixtures
   so all three engines can be compared with the same coordinates and explicit
   camera/theme/form settings.

## 9. Current working-tree continuation checkpoint

Two passes have run against this map. The first closed the profile-input and
adaptive-budget seams and tightened shared interaction state. The second closed
the renames, bond geometry, soft-union reachability, automatic presets, and the
serialized pick union. The remaining visual-parity items, all of which need a
comparison fixture or a measurement, stay open.

### Completed in the second pass

- **Crate and module renames:** `molgfx-api` → `molgfx-scene` (directory,
  workspace member and dependency, lockfile, dependents, `AGENTS.md`, README,
  guides) and `science` → `overlay` (`overlay/`, `overlay/overlay_ops.rs`,
  `patch/plan/overlay.rs`, `overlay_binding.rs`, the interaction spec and ID
  types, `Scene::overlay_handles`, `SceneSpec.interactions`, the Python
  bindings and stubs, the docs page). No engine concept names science; the
  remaining PyPI trove classifiers classify the package.
- **Multi-bond geometry:** the packed `BondGpu.order` and `variant` are now
  read by the shader. A double bond draws two parallel strands and a triple
  bond three, packed inside the width one stick of that radius occupies; an
  aromatic bond is a solid line with a broken inner line, and order 0 stays
  dotted. The default stick radius is the reference 0.25 Å.
- **Soft-union surface:** `SurfaceStyle::SoftUnion` is reachable from the scene
  enum, the command `style=soft_union` and Python, so the blob surface the WGSL
  already renders is authorable.
- **Automatic presets:** the size-based policy is now reachable as the `auto`
  authoring command and `molgfx.Scene.auto()`, not only the Rust
  `Scene::add_auto`.
- **Serialized pick union:** `ResolvedPick` carries `label`, `measurement` and
  `volume_segment` variants with their own records, and a measurement has its
  own `EntityKind::Measurement` GPU namespace so it no longer collides with an
  annotation's storage row.

### Verification observed in this pass

- `nix develop -c cargo fmt --all --check`
- `nix develop -c cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean
- `nix develop -c cargo test --workspace --all-features`: 1207 passed, 0 failed
- `nix develop -c cargo check` and `clippy -p molgfx-wasm --target wasm32-unknown-unknown`: clean
- `maturin develop` + `python -m unittest discover -s python/tests`: 67 passed
- `python -m mypy.stubtest molgfx._engine`: no issues
- Repository-policy greps (lint suppression, non-test `unwrap`, `unsafe`, the
  500-line cap, manifest lint allowances, the `benchmarks/` seam): all empty

A native render of a C=O plus aromatic fixture confirms the double bond draws
as two strands. The earlier browser comparison remains exploratory evidence;
no golden or cross-engine fixture was added in this pass.

### Recommended continuation order

1. Add symmetry-mate/inter-unit bond records at the MolFrame/core instance
   topology boundary, including source-endpoint picking.
2. Add a marker mask/edge pass with ghosted occluded edges, then replace
   per-request hover readback with one invalidated low-resolution buffer.
3. Extend the single property/input registry with named occupancy,
   hydrophobicity, uncertainty, molecule-type, charge, carbon-by-chain, and
   sequence-rainbow schemes; add matching legends and fixtures.
4. Finish the dedicated blob/ellipsoid/orientation/polyhedron/plane/unit-cell
   *forms* and the licorice junction/seam and lines lone-atom cross.
5. Measure temporal convergence, multiscale AO, AA selection, sphere LOD/Hi-Z,
   upload budgets, WASM cold start, and same-input protein/nucleic/ligand/
   density comparisons before changing thresholds or committing goldens.
