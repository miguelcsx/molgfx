//! Cost of per-frame interaction edits and selection churn against scene size.
//!
//! Hover moves every frame, so its cost must follow the picked rows rather
//! than the molecule; the groups here walk atom counts so a linear term shows
//! up as a rising per-atom figure while a flat one stays flat.

use criterion::{BenchmarkId, Criterion, Throughput};
use molgfx_bench::synthetic;
use std::hint::black_box;

/// Residue counts: 2k, 8k, 32k and 128k atoms.
const SIZES: [usize; 4] = [400, 1_600, 6_400, 25_600];
const ATOMS_PER_RESIDUE: u64 = 5;

fn core_scene(residues: usize) -> molgfx_core::Scene {
    match molgfx_core::Scene::from_structure(&synthetic::structure(residues)) {
        Ok(scene) => scene,
        Err(error) => panic!("synthetic scene must resolve: {error}"),
    }
}

/// A pointer hover that moves between two atoms, as every frame of a drag does.
fn hover_scaling(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("interaction/hover_move");
    for residues in SIZES {
        let mut scene = core_scene(residues);
        let Some((handle, _)) = scene.structures().next() else {
            panic!("the synthetic scene has a structure")
        };
        let atoms = residues as u64 * ATOMS_PER_RESIDUE;
        group.throughput(Throughput::Elements(atoms));
        let _ = group.bench_function(BenchmarkId::from_parameter(atoms), |bencher| {
            let mut row = 0_u32;
            bencher.iter(|| {
                row = if row == 0 { 7 } else { 0 };
                let moved = scene.update_pointer_hover(Some(handle), black_box(&[row]));
                black_box(moved.is_ok())
            });
        });
    }
    group.finish();
}

/// A channel edit evaluates a query once; its handle must not outlive it.
///
/// Alternating two queries makes every edit a fresh evaluation, so a handle
/// leaked per edit shows as growth in the stored-selection table over the run.
fn channel_edit_scaling(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("interaction/channel_edit");
    for residues in SIZES {
        let structure = synthetic::structure(residues);
        let mut scene = match molgfx::Scene::from_structure(&structure) {
            Ok(scene) => scene,
            Err(error) => panic!("synthetic scene must resolve: {error}"),
        };
        let atoms = residues as u64 * ATOMS_PER_RESIDUE;
        group.throughput(Throughput::Elements(atoms));
        let _ = group.bench_function(BenchmarkId::from_parameter(atoms), |bencher| {
            let mut flip = false;
            bencher.iter(|| {
                flip = !flip;
                let query = if flip { "name CA" } else { "name CB" };
                let edit = scene.set_interaction(
                    molgfx::InteractionChannel::Selected,
                    Some(black_box(query).into()),
                );
                black_box(edit.is_ok())
            });
        });
    }
    group.finish();
}

/// Representation counts for the structural-patch groups.
const REPRESENTATIONS: [usize; 4] = [64, 256, 1_024, 4_096];

/// A scene with `count` cartoons, each over its own residue, added in one patch.
fn crowded_scene(count: usize) -> molgfx::Scene {
    let structure = synthetic::structure(count + 16);
    let mut scene = match molgfx::Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("synthetic scene must resolve: {error}"),
    };
    let staged = scene.transaction(|transaction| {
        for residue in 1..=count {
            let _ = transaction.add(molgfx::rep::cartoon(format!("resid {residue}").as_str()))?;
        }
        Ok(())
    });
    if let Err(error) = staged {
        panic!("crowded scene must stage: {error}")
    }
    scene
}

/// Adding then removing one representation among many distinct selections.
///
/// Each edit is a structural patch. A patch that re-resolves every
/// representation grows with the count; one that touches only the changed
/// representation stays flat, and so does the selection cache beyond its
/// former 256-entry bound.
fn structural_patch_scaling(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("patch/add_remove_representation");
    for count in REPRESENTATIONS {
        let mut scene = crowded_scene(count);
        group.throughput(Throughput::Elements(count as u64));
        let _ = group.bench_function(BenchmarkId::from_parameter(count), |bencher| {
            bencher.iter(|| {
                let added = scene.add(molgfx::rep::cartoon("resid 5"));
                if let Ok(id) = added {
                    let removed = scene.transaction(|transaction| transaction.remove(id));
                    black_box(removed.is_ok());
                }
            });
        });
    }
    group.finish();
}

criterion::criterion_group!(
    interaction,
    hover_scaling,
    channel_edit_scaling,
    structural_patch_scaling
);
criterion::criterion_main!(interaction);
