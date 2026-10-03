# Browser SDK architecture and viewer postmortem

## Ownership

MolGFX is one repository with three public library surfaces: Rust, Python and the
browser SDK. The Rust engine remains usable headlessly. `crates/molgfx-wasm` is
only the binding; handwritten browser orchestration belongs to `web/`.

| Owner | Responsibility |
|---|---|
| MolFrame | Parsing, molecular identity, query language, scientific data |
| MolGFX Rust | Scene, representations, appearance, interaction channels, commands, camera mathematics, GPU rendering, semantic picking |
| WASM binding | ABI conversion over the curated Rust facade |
| Browser Viewer | Canvas, capabilities, input normalization, DPR, scheduling, asynchronous work and disposal |
| React | Component lifecycle around the same Viewer |
| AnyWidget | Revisioned scene/binary transport around the same Viewer |
| MolStation | Projects, panels, console, file browser, workflows and product state |

## Historical review

The removal commit `5b9236c` correctly separated the Rust binding from browser
orchestration and removed application UI. It incorrectly treated the reusable
canvas host itself as an application that must leave the repository. The backup
records the removed runtime at `507eae5`; the earlier `f9469e47` browser contract
already separates `SceneSource`, `ViewerSink` and inline runtime transport.
Camera primitives were subsequently exposed through the public bindings rather
than duplicated as browser-only camera mathematics.

| Former component | Decision and reason |
|---|---|
| `ViewerHost`, `SceneSource`, `ViewerSink` | Adapt the source/sink boundary, separate local authority from external projection |
| `RenderLoop` | Adapt demand-driven scheduling and detached readback; invalidate against the complete view, not only scene replacement |
| Lifecycle cleanup | Recover deterministic listeners/observers/RAF/resource teardown |
| Input/controller bridge | Recover browser normalization; Rust still computes camera movement |
| Runtime loader | Recover shared initialization and inline notebook loading; defer browser requirements until creation |
| Scene patch adapter | Recover revision checking and full resynchronization |
| Local command backend | Recover Session ownership, not completion/history UI or another parser |
| AnyWidget transport | Recover compact BinaryCIF payloads, source deduplication and revision protocol |
| Build script | Move to `web/scripts`; build one content-keyed runtime and copy the artifacts unchanged |
| Mount/Workbench/console/toolbar/file input | Do not restore; these are application UX |

### Defects not to carry forward

1. The old viewer reconstructed an atom selection from residue number and chain
   strings. This loses source identity, insertion codes, alternate locations and
   instance provenance. The new pick event preserves the complete engine result;
   it does not infer a query or silently mutate selection.
2. The old loop blocked frame requests during a detached readback despite the
   readback no longer borrowing the renderer. Camera, resize and scene changes
   must remain possible and invalidate stale asynchronous results.
3. A scene epoch alone does not validate a pick after a camera or target-size
   change. View identity covers scene, camera and physical target changes.
4. Disposal must stop RAF/listeners immediately, then release resources after
   outstanding readback work settles. A completion cannot resurrect scheduling.
5. Legacy flattened pick traits and console-specific contracts are not a stable
   SDK boundary. There is no compatibility obligation to the removed shell.
6. Copying the historical dynamic import verbatim is insufficient evidence of
   npm bundler compatibility. Package assets must be exercised through an actual
   npm consumer, not only a source-served page.
7. Browser smoke exposed two native frame-demand defects: a skipped frame could
   stop scheduling before presenting the requested scene, and successful scene
   synchronization never recorded the representation revision, restarting
   temporal convergence indefinitely. The Rust frame report now requests
   skipped/upload/convergence work and synchronization records its revision.
   A regression verifies convergence, idle and restart after camera movement;
   the notebook browser scenario checks visible pixels before any pick.

## Current engine inventory

The design reuses these existing capabilities rather than defining new semantics:

- `SceneSpec` is portable JSON; `ScenePatch` is atomic and base-revision checked.
  `Scene.apply` applies the same canonical patch to the resolved scene.
- `Session` owns symbolic names, layers, aliases and history. `Command` is typed
  Rust IR; the existing command-text parser builds that IR. Browser convenience
  operations use this existing Session boundary, not a JS grammar.
- The `auto` command delegates to `molgfx_scene::preset::auto_representations`.
  It registers actual engine-selected forms as named Session layers.
- Interaction channels are canonical scene fields: selected, hovered, focused,
  muted, hidden and custom channels. Selection remains the MolFrame query model.
- `Scene.frame` and `Scene.camera` compute framing in Rust. `Camera.project` and
  `Camera.ray` are public general primitives. Arcball, Orbit and Fly controllers
  accept abstract pointer/button/scroll/pinch/key events in Rust.
- `Renderer` owns WebGPU resources and physical target size. Rendering returns
  whether another frame is required; `frameReportJSON` exposes observed engine
  frame metadata rather than a host-invented quality policy.
- `beginPick` returns a detached readback; resolving it holds no renderer borrow.
  `finishPick` resolves semantic provenance against the live scene.
- Resolved atom/bond picks carry source/structure, dataset, chunk, topology
  revision and source row metadata. Other pick namespaces remain distinct.
- Python `Scene.browser_sources()` materializes compact structural bytes lazily;
  placed sources share the provider encoding. `subscribe`/`unsubscribe` are the
  explicit weak scene-patch transport boundary.

The current selection contract is query-based, not a provenance-aware loci API.
A safe general pick-to-selection primitive does not currently exist. Exposing
complete picks and explicit `select(query)` is honest; reconstructing chain/resid
queries would not be. A future exact loci primitive belongs in MolGFX Rust and
MolFrame, never in an adapter's string heuristics.

## Distribution decisions

- One npm package, `molgfx`, not an npm workspace of independent public packages.
- Root and viewer subpaths expose the browser API; the React subpath is optional.
  React is an optional peer and is not imported by the core viewer.
- AnyWidget is built from the same source/build as the browser SDK. Its transport
  entry is not a separate product or a second renderer.
- AnyWidget is a normal Python dependency for `pip install molgfx` usability.
  The top-level Python package loads the Viewer lazily; ordinary `import molgfx`
  must not initialize notebook machinery.
- Cargo workspace, Python and npm versions are coupled and checked at build time.
  Package metadata is the version source, not literals spread through examples.
- Generated bindings and distribution assets are untracked build outputs.
  npm and Python package the identical JS glue/WASM pair. The docs import the
  public npm package and their bundler emits its WASM asset; there is no private
  runtime-directory consumer.
- The manifest identifies the glue/WASM pair by content hash and records artifact
  hashes. Distribution checks compare real archive contents, not filenames alone.

## Semantic authority

A browser-local viewer owns one WASM Scene and Session. Loading a structure
creates a fresh scene/session and uses the engine `auto` command; replacing a
structure does not merge hidden host state or keep aliases from another source.
Convenience edits run through the Session and request rendering of committed
state.
Current selection is the Rust `Command::SetSelection` transaction (`selection
QUERY`, or `selection` to clear), not a named-selection definition followed by
an out-of-band scene edit. It updates the selected overlay and `$sel` together,
participates in undo/redo, and rolls back with any failing statement in a program.


For Python, the supplied or created Python Session/Scene is authoritative. A raw
Structure gets a new Session and an explicit canonical `auto` command. An
existing Scene or Session is shared without applying `auto` or otherwise
surprising its owner. The browser retains only a resolved rendering projection.

Full synchronization sends canonical scene JSON, revision and compact binary
sources. Incremental synchronization sends the committed patch with its base
revision. A projection that cannot apply a patch must request a full resync;
it must not forge a revision, replay the command independently, or accept a
partial scene. Source changes are announced with the corresponding source bytes.
Browser picks report canonical metadata, and explicit selection changes are
committed by the authority. Browser camera movement is host view state unless
explicitly authored into the scene.

## Lifecycle contract

Module imports are SSR-safe. Browser/WebGPU requirements are checked at creation,
not import. Runtime initialization is shared across viewers; renderer and scene
resources belong to each viewer. A view tracks its own scene and view generations,
so asynchronous work from an old scene or view cannot publish into a replacement.

Render scheduling follows the engine's demand signal, coalesces redraw requests,
and uses the physical DPR-sized render target. No idle continuous loop is needed
for a settled scene. Observers and listeners have explicit cleanup ownership.
Disposal is idempotent, stops new work immediately, invalidates pending results,
and awaits outstanding work before freeing WASM resources.

## Verification obligations

Source typechecking and a successful WASM build are not browser proof. Release
verification exercises real public create/load/render/resize/input/pick/edit and
dispose boundaries, including multiple viewers and pending-work teardown. React
mount/update/unmount and AnyWidget projection/resync exercise the same Viewer.
SSR imports and an actual npm bundler consumer verify packaging boundaries.
A built wheel must install, construct a Viewer, propagate patches, and contain
the identical browser runtime checked against the npm archive manifest.

Browser compatibility claims are capability-based. Chromium evidence does not
prove Firefox, Safari or every GPU/driver combination. Unsupported or unavailable
browser environments must be reported explicitly, not counted as passing.

## Initial SDK verification and measurements

On the macOS arm64 workstation, the release build, strict TypeScript, ten
lifecycle/revision unit tests, three artifact tests, Python lint/type/stub checks
and 69 Python tests passed. Rust workspace tests, warning-denying Clippy, WASM
target Clippy, rustdoc and benchmark compilation passed. Benchmark compilation
is not a measured engine benchmark.

Playwright 1.63.0 with full Chromium 153.0.8010.12 and
`--enable-unsafe-webgpu` exercised real GPU render/pick, camera input, repeated
load/resize/dispose, independent viewers and pending-readback disposal. A direct
browser smoke additionally observed a 640×480 CSS canvas at 1280×960 physical
pixels (DPR 2), complete atom provenance, a rendered screenshot and zero canvas
elements after disposal. Production consumers installed the actual npm archive
and initialized the viewer through Vite 6.4.3 and Next 16.3.5 webpack output.
The documentation site built 54 pages through Next Turbopack.

The initially verified runtime has content key `7f622ab1d32efbed`; its WASM SHA256 is
`4436352422c3cf9325ac95c03c5b54cedd4366fe8cf7d411c3c29448497ed188`.
Actual npm and native-wheel archives matched every manifest artifact by bytes,
hash and version. WASM is 8,685,961 bytes (2,551,914 gzip); the core index is
15,167 bytes (5,516 gzip). JavaScript sizes are entry-bundle sizes, not total
React or WASM transfer size.

Five fresh Python processes measured median core import at 5.991 ms and lazy
Viewer class loading at 69.951 ms. No notebook imports occur during core import.
These are initial SDK measurements, not a before/after speedup claim.

| Synthetic atoms | Create/load ms | Three commands ms | Host/GPU-ready ms | Twelve camera inputs ms | Idle submissions / 300 ms |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 64.3 | 18.2 | 130.7 | 200.8 | 0 |
| 10,000 | 33.7 | 7.8 | 49.4 | 200.4 | 0 |
| 50,000 | 61.6 | 31.4 | 94.0 | 204.1 | 0 |

These single-run host timings include different cache warmness; they are not
device timestamps, a scaling study, representative protein performance or a
claimed speedup. The small two-atom loads were 56.4/10.2 ms and the semantic pick
was 7.5 ms. All eight browser scenarios passed; idle and post-disposal submission
counts were zero over 300 ms, with zero post-disposal pick callbacks. The
reproducible scenarios live under `web/tests/browser`.

### Coverage limits and release constraints

- Safari, Firefox, Linux SwiftShader and Windows were not exercised here.
- Old macOS Chromium headless-shell lost the GPU device during multi-viewer and
  resize scenarios; full Chromium passed without weakening assertions.
- Real JupyterLab/Notebook/Colab application integration is distinct from the
  Python-backed AnyWidget transport browser scenario; neither browser unit
  fixtures nor Python traits alone prove every notebook frontend.
- Local Nix-built native wheels reference external libiconv/libcharset; archive
  equality does not prove wheel portability. Platform repair/release CI remains
  necessary before distribution. Maturin 1.12.6 failed native dependency linkage
  here; the verified 1.15 builder is now the build-system minimum.
- Initial SDK verification did not publish npm or Python packages.
- Exact pick-to-selection loci do not exist in the current engine. Rich pick
  events and explicit query selection are implemented; click-to-residue identity
  heuristics are deliberately absent.

## Distribution review for 0.4.1

The scoped release snapshot excludes concurrent secondary-structure and palette
work. Workspace formatting, warning-denying Clippy and tests, plus real-target
WASM checking and Clippy, passed against the pinned dependency.

The repaired macOS arm64 wheel grafts libiconv/libcharset and contains no
`/nix/store` dependency references. Both the direct wheel and a wheel rebuilt
from the extracted source archive passed all 69 Python tests in fresh installs.
The source rebuild blocked Node, npm, npx and wasm-pack and had no adjacent
MolFrame checkout. Native rendering produced a 64×64 image and a projected atom
landed on drawn geometry. Nine fail-closed packaging gate tests passed.

All 31 hashed browser assets matched between npm, sdist and both wheels. Runtime
content key: `6c45bab28027dbde`. WASM SHA256:
`473956b4647c7edd9899ca80a2ff685ae2ecd3a3d2a7009858ac5b80ae7ae779`.

The production documentation built 54 pages. Its real-browser smoke rendered
34,725 colored haemoglobin pixels, fetched byte-identical WASM with HTTP 200
under `/molgfx/_next/`, and unmounted the canvas on navigation without page
errors. Packed Vite and Next consumers, SSR imports, private-export rejection,
npm publication dry-run and release/pages workflow lint passed.

The npm registry returned 404 for `molgfx`, and local `npm whoami` returned
`ENEEDAUTH`. First publication requires maintainer authentication; subsequent
trusted publishing names `miguelcsx/molgfx`, `release-npm.yml`, environment
`npm`. There is no token fallback. Linux and Windows wheel portability remains
a platform-release CI obligation, not a claim from the macOS smoke.
