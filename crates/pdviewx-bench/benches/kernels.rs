//! Criterion coverage for the CPU kernels a caller hits through the facade,
//! measured on a real structure (haemoglobin, 4hhb): scene construction
//! (columnar tables, spatial BVH, hierarchy) and the spatial-selection path
//! that clones a roaring result per structure. GPU-side frame cost lives in
//! the `frame_profile` binary instead — criterion drives no device here.

use criterion::{Criterion, Throughput};
use pdviewx::{AtomSelection, Scene};
use std::hint::black_box;

const STRUCTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../benchmarks/scenes/4hhb.cif"
);

fn load() -> pdbiox::Structure {
    match pdbiox::read(STRUCTURE) {
        Ok(structure) => structure,
        Err(diagnostics) => panic!("fixture {STRUCTURE} must parse: {diagnostics:?}"),
    }
}

/// Atom indices of every copy of a named component (e.g. the four haems).
fn component_atoms(structure: &pdbiox::Structure, name: &str) -> Vec<u32> {
    structure
        .data()
        .residues()
        .filter(|residue| residue.name() == Some(name))
        .flat_map(|residue| residue.atoms().map(|atom| atom.index().get()))
        .collect()
}

fn bench_kernels(c: &mut Criterion) {
    let structure = load();
    let atom_count = structure.positions().len() as u64;

    let mut group = c.benchmark_group("core");
    group.throughput(Throughput::Elements(atom_count));

    // Full per-load construction: columnar atom table, spatial BVH, hierarchy.
    group.bench_function("scene_build", |b| {
        b.iter(|| black_box(Scene::from_structure(black_box(&structure))));
    });

    // The spatial neighbourhood query used by focus: BVH sphere tests plus the
    // roaring-result build, over the haem ligands.
    let ligand = component_atoms(&structure, "HEM");
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("scene must build: {error:?}"),
    };
    let reference = scene.add_selection(AtomSelection::Sparse(ligand));
    group.bench_function("select_residues_within_5A", |b| {
        b.iter(|| {
            let handle = scene.select_residues_within(black_box(reference), 5.0);
            black_box(handle.is_ok())
        });
    });
    group.finish();
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    bench_kernels(&mut criterion);
    criterion.final_summary();
}
