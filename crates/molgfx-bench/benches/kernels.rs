//! Criterion coverage for semantic scene construction and canonical selection
//! resolution on a real structure (haemoglobin, 4hhb).

use criterion::{Criterion, Throughput};
use molgfx::Scene;
use molgfx_bench::fixtures;
use std::hint::black_box;
use std::path::Path;

const STRUCTURE: &str = "4hhb.cif";

/// The measured structure, or nothing when this checkout carries no scene
/// corpus. A checkout without one is a normal state, so the group is skipped
/// with a note instead of aborting the run.
fn load() -> Option<molframe::Structure> {
    let path = fixtures::scene(Some(Path::new(STRUCTURE)))?;
    if !path.is_file() {
        eprintln!(
            "skipped: {} is not in the scene corpus; set MOLGFX_SCENES to point at one",
            path.display()
        );
        return None;
    }
    match molframe::read(&path) {
        Ok(structure) => Some(structure),
        Err(diagnostics) => {
            eprintln!("skipped: {} must parse: {diagnostics:?}", path.display());
            None
        }
    }
}

fn bench_kernels(c: &mut Criterion) {
    let Some(structure) = load() else {
        return;
    };
    let atom_count = u64::try_from(structure.positions().len())
        .into_iter()
        .fold(u64::MAX, |_, value| value);

    let mut group = c.benchmark_group("core");
    group.throughput(Throughput::Elements(atom_count));

    // Full per-load construction: columnar atom table, spatial BVH, hierarchy.
    group.bench_function("scene_build", |b| {
        b.iter(|| black_box(Scene::from_structure(black_box(&structure))));
    });

    let representation = molgfx::rep::spacefill(molgfx::sel::resname().eq("HEM"));
    group.bench_function("canonical_selection_and_representation", |b| {
        b.iter(|| {
            let result = match Scene::from_structure(black_box(&structure)) {
                Ok(mut scene) => scene.add(black_box(representation.clone())),
                Err(error) => Err(error),
            };
            black_box(result)
        });
    });
    group.finish();
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    bench_kernels(&mut criterion);
    criterion.final_summary();
}
