//! Cost scaling in scene size, to separate linear work from quadratic.
//!
//! The everyday-operation benches span a ligand, a domain and a small complex.
//! This one goes past that: it walks residue counts across four doublings, so a
//! path that is linear reports a constant per-atom cost, and one that is
//! quadratic reports a cost that doubles with every doubling. The point is not
//! an absolute number — machine noise moves those — but the *ratio* between
//! consecutive sizes, which is stable because each size is measured on the same
//! machine in the same run.
//!
//! Input is generated in memory, so this runs on any checkout.

use criterion::{BenchmarkId, Criterion, Throughput};
use molgfx::Scene;
use molgfx_bench::synthetic;
use std::hint::black_box;

/// Residue counts across four doublings: 2k, 8k, 32k and 128k atoms.
///
/// The upper end is deliberately past the sizes the everyday benches reach,
/// because a quadratic term is invisible until the sizes are far apart.
const SIZES: [usize; 4] = [400, 1_600, 6_400, 25_600];

/// Atoms per generated residue, matching `synthetic::ATOMS`.
const ATOMS_PER_RESIDUE: u64 = 5;

fn atoms(residues: usize) -> u64 {
    residues as u64 * ATOMS_PER_RESIDUE
}

fn scene_of(residues: usize) -> Scene {
    let structure = synthetic::structure(residues);
    match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("synthetic scene must resolve: {error}"),
    }
}

/// The one representation the generated scene carries.
fn representation_of(scene: &Scene) -> molgfx::RepresentationId {
    match scene.spec().representations.keys().next().copied() {
        Some(id) => id,
        None => panic!("the synthetic scene has a representation"),
    }
}

/// Constructing a scene is inherently proportional to the molecule.
///
/// A constant per-atom cost across four doublings is the signature of linear
/// work; a rising one is a quadratic term in the table build.
fn construction_scaling(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("scaling/construction");
    for residues in SIZES {
        let structure = synthetic::structure(residues);
        group.throughput(Throughput::Elements(atoms(residues)));
        let _ = group.bench_function(BenchmarkId::from_parameter(atoms(residues)), |bencher| {
            bencher.iter(|| black_box(Scene::from_structure(black_box(&structure))));
        });
    }
    group.finish();
}

/// An appearance edit must stay flat: it is a flag and a uniform, not a walk.
///
/// This is the invariant most easily broken by a change that starts deriving
/// something per representation from the molecule, so it is measured across
/// sizes as well as across representation counts.
fn appearance_scaling(criterion: &mut Criterion) {
    use molgfx::{rep, sel};
    let mut group = criterion.benchmark_group("scaling/appearance_edit");
    for residues in SIZES {
        let mut scene = scene_of(residues);
        let _ = scene.add(rep::spacefill(sel::all()));
        let id = representation_of(&scene);
        group.throughput(Throughput::Elements(atoms(residues)));
        let _ = group.bench_function(BenchmarkId::from_parameter(atoms(residues)), |bencher| {
            let mut opacity = 0.1_f32;
            bencher.iter(|| {
                opacity = if opacity > 0.9 { 0.1 } else { opacity + 0.01 };
                match scene.set_opacity(black_box(id), black_box(opacity)) {
                    Ok(()) => {}
                    Err(error) => panic!("an opacity edit must apply: {error}"),
                }
            });
        });
    }
    group.finish();
}

/// A selection resolves against the molecule, so its cost is proportional to it.
///
/// The question here is only whether the constant is a walk or something worse:
/// a query that re-scans per representation, or one that rebuilds an index,
/// shows up as a rising per-atom cost across the doublings.
fn selection_scaling(criterion: &mut Criterion) {
    use molgfx::{rep, sel};
    let mut group = criterion.benchmark_group("scaling/selection");
    for residues in SIZES {
        let mut scene = scene_of(residues);
        group.throughput(Throughput::Elements(atoms(residues)));
        let _ = group.bench_function(BenchmarkId::from_parameter(atoms(residues)), |bencher| {
            bencher.iter(|| {
                let result = scene.add(black_box(rep::spacefill(sel::all())));
                black_box(result.is_ok())
            });
        });
    }
    group.finish();
}

/// Many representations over one selection must not multiply per-atom work.
///
/// With eight representations the per-atom cost should still match the
/// single-representation figure, because they share one record set and one
/// visibility key. A rising curve means the sharing regressed into a per-slot
/// walk of the molecule.
fn representation_count_scaling(criterion: &mut Criterion) {
    use molgfx::{rep, sel};
    let mut group = criterion.benchmark_group("scaling/representations");
    let residues = SIZES[2];
    for count in [1_usize, 2, 4, 8] {
        let mut scene = scene_of(residues);
        for _ in 0..count {
            let _ = scene.add(rep::spacefill(sel::all()));
        }
        let id = representation_of(&scene);
        group.throughput(Throughput::Elements(atoms(residues)));
        let _ = group.bench_function(BenchmarkId::from_parameter(count), |bencher| {
            let mut opacity = 0.1_f32;
            bencher.iter(|| {
                opacity = if opacity > 0.9 { 0.1 } else { opacity + 0.01 };
                match scene.set_opacity(black_box(id), black_box(opacity)) {
                    Ok(()) => {}
                    Err(error) => panic!("an opacity edit must apply: {error}"),
                }
            });
        });
    }
    group.finish();
}

/// Where the construction cost goes, before optimising any of it.
///
/// One structure is built once and then reused, so this separates the two
/// per-atom passes a construction pays from everything else it does.
fn construction_breakdown(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("scaling/construction_parts");
    for residues in [SIZES[1], SIZES[3]] {
        let source = synthetic::structure(residues);
        group.throughput(Throughput::Elements(atoms(residues)));
        let _ = group.bench_function(BenchmarkId::new("clone_structure", atoms(residues)), |b| {
            b.iter(|| black_box(black_box(&source).clone()));
        });
        let _ = group.bench_function(BenchmarkId::new("molecular_source", atoms(residues)), |b| {
            b.iter(|| {
                black_box(molgfx_core::MolecularSource::from_molframe(black_box(
                    &source,
                )));
            });
        });
        let _ = group.bench_function(BenchmarkId::new("asset", atoms(residues)), |b| {
            b.iter(|| {
                let source = molgfx_core::MolecularSource::from_molframe(black_box(&source));
                let built = molgfx_core::StructureAsset::from_source(
                    molgfx_core::DatasetId::LEGACY,
                    source,
                );
                black_box(built.is_ok());
            });
        });
        let asset = match molgfx_core::StructureAsset::new(molgfx_core::DatasetId::LEGACY, &source)
        {
            Ok(asset) => asset,
            Err(error) => panic!("asset builds: {error}"),
        };
        let _ = group.bench_function(BenchmarkId::new("placed_structure", atoms(residues)), |b| {
            b.iter(|| black_box(molgfx_core::PlacedStructure::from_asset(black_box(&asset))));
        });
        let _ = group.bench_function(BenchmarkId::new("core_scene", atoms(residues)), |b| {
            b.iter(|| black_box(molgfx_core::Scene::from_structure(black_box(&source)).is_ok()));
        });
        let _ = group.bench_function(
            BenchmarkId::new("core_scene_from_asset", atoms(residues)),
            |b| b.iter(|| black_box(molgfx_core::Scene::from_asset(black_box(&asset)))),
        );
        let facade_source = molgfx_core::MolecularSource::from_molframe(&source);
        let _ = group.bench_function(BenchmarkId::new("structure_hash", atoms(residues)), |b| {
            b.iter(|| black_box(molgfx_api::structure_hash(black_box(&facade_source)).len()));
        });
        let _ = group.bench_function(
            BenchmarkId::new("facade_from_source", atoms(residues)),
            |b| b.iter(|| black_box(Scene::from_source(black_box(facade_source.clone())).is_ok())),
        );
        let _ = group.bench_function(BenchmarkId::new("full_scene", atoms(residues)), |b| {
            b.iter(|| black_box(Scene::from_structure(black_box(&source)).is_ok()));
        });
    }
    group.finish();
}

criterion::criterion_group!(
    scaling,
    construction_scaling,
    construction_breakdown,
    appearance_scaling,
    selection_scaling,
    representation_count_scaling,
);
criterion::criterion_main!(scaling);
