# pdviewx

**A semantic, GPU-native molecular rendering engine in Rust.** pdviewx turns a
molecular structure into an interactive scene where the *biology* controls the
picture — what is emphasised, how it is lit, what you can click — instead of
treating a molecule as a bag of coloured spheres.

![Status: specification](https://img.shields.io/badge/status-specification-orange)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-000000?logo=rust)
![Backend: wgpu](https://img.shields.io/badge/backend-wgpu%20%2B%20WGSL-blue)

> **This repository is specification-first and pre-implementation.** The
> normative design lives under [`docs/`](docs/) and is complete; the engine
> itself is being built against it. See [`ROADMAP.md`](docs/ROADMAP.md).

---

## Why pdviewx

Every current molecular viewer answers the same question: *given coordinates,
draw an image.* PyMOL, ChimeraX, VMD and Mol\* are excellent at it. But they
treat a structure as geometry, and the scientist is left to reconstruct the
meaning by hand — which residues line the pocket, why this pose scores, where the
model is uncertain.

pdviewx inverts the pipeline:

```
coordinates → image                          (every other viewer)
scientific knowledge → semantic scene → GPU rendering → understanding   (pdviewx)
```

The molecule carries its own semantics — secondary structure, binding sites,
interactions, confidence, chemistry — and those semantics *drive* the
representation, the illumination and the interactivity. Select a ligand and the
engine fades the bulk of the protein, raises the pocket surface, draws the
hydrogen bonds and labels the residues that matter — because it knows what a
ligand and a pocket are, not because you scripted forty commands.

The flagship it is built to win: **the best renderer in the world for
understanding *why* a protein–ligand pose scored the way it did.**

## What it is, and is not

pdviewx **is** a rendering engine delivered as a Rust library. It renders; it
does not fetch, dock, simulate or predict. Structure parsing and the structural
data model come from its sibling project [`pdbiox`](https://github.com/miguelcsx/pdbiox);
pdviewx never reimplements them. There is no bundled application — the engine is
embeddable, and `examples/` shows it driving a `winit` window.

## Features

- **Meshless impostor rendering** — atoms and bonds are analytic spheres and
  capsules intersected in the fragment shader. No tessellation, exact
  silhouettes at any zoom, a point of memory per atom.
- **GPU-generated cartoons** — ribbons extruded along Cα splines using
  parallel-transport frames, with distinct α-helix, β-sheet and coil profiles.
- **Molecular surfaces and volumes** — solvent-accessible and solvent-excluded
  surfaces and cryo-EM density as implicit fields ray-marched on the GPU.
- **Two render modes** — a 60/120 fps realtime raster path, and a progressive
  quality mode that path-traces ambient occlusion and soft shadows when the
  camera settles.
- **The semantic layer** — focus+context, distance-banded emphasis,
  materials that encode charge and hydrophobicity, confidence-driven rendering,
  interaction edges, and level of detail that walks atoms → residues →
  secondary structure → domains as the camera pulls back.
- **Zero-copy data path** — `pdbiox`'s `N × 3 f32` coordinate column uploads
  straight to a GPU buffer with no repack.

## Getting started

```rust
use pdviewx::{Engine, Representation, Select};

// pdbiox parses; pdviewx renders.
let structure = pdbiox::read("1abc.cif")?;
let mut scene = Engine::scene_from(&structure);

scene.represent(Select::polymer(), Representation::Cartoon);
scene.represent(Select::ligands(), Representation::BallAndStick);
scene.focus(Select::ligands());          // the semantic layer does the rest

let frame = engine.render(&scene, &camera)?;
```

Run an example against a live window:

```bash
cargo run --example pocket --release
```

## The semantic layer — the part no other renderer has

Other engines expose *representations* and leave the composition to you. pdviewx
exposes *intent*. `scene.focus(ligand)` is not a preset; it is a query into the
semantic scene graph — nearby residues, the pocket boundary, the interaction
network — resolved into a coherent, legible picture with the context faded but
present. The same graph answers `scene.find(HydrogenBonds(ligand))` and drives
uncertainty rendering, differential rendering between two poses, and semantic
level of detail. This is documented in
[`docs/12-semantic-rendering.md`](docs/12-semantic-rendering.md).

## Status

Specification complete; implementation beginning at
[`ROADMAP.md`](docs/ROADMAP.md) Phase 1 (the realtime impostor path). Every
capability in [`PARITY.md`](docs/PARITY.md) is marked `planned` or `β`; a `✓`
requires a golden scene and a passing perceptual-diff gate, which no checkout yet
carries. Claims here are honest about that.

## Repository layout

```
pdviewx/
├── AGENTS.md            contributor + agent contract
├── RULES.md             binding code style guide
├── README.md
├── docs/                the normative specification (00–26, meta, adr/)
├── crates/
│   ├── pdviewx-math/        layer 0  math foundation
│   ├── pdviewx-core/        layer 0  semantic scene graph
│   ├── pdviewx-gpu/         layer 1  renderer HAL
│   ├── pdviewx-gpu-wgpu/    layer 2  default backend
│   ├── pdviewx-shaders/     layer 2  shared WGSL
│   ├── pdviewx-geometry/    layer 3  GPU geometry generation
│   ├── pdviewx-render/      layer 4  render graph + modes
│   ├── pdviewx-semantic/    layer 5  semantic rendering engine
│   ├── pdviewx/             layer 6  facade
│   └── pdviewx-bench/       benchmark harness
├── examples/            winit-driven illustrations
└── brainstorming/       non-normative ideation (provenance)
```

## Contributing

Read [`AGENTS.md`](AGENTS.md) for the crate map and the verification loop, and
[`RULES.md`](RULES.md) before writing code. Every change is traceable to a
requirement or an ADR under [`docs/`](docs/).

## License and citation

Dual-licensed under **MIT OR Apache-2.0**, at your option. pdviewx builds on
`pdbiox` and is designed for the same open, reproducible, provenance-first
science. See [`docs/26-governance.md`](docs/26-governance.md).
