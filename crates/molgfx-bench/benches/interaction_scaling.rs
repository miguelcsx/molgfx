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

criterion::criterion_group!(interaction, hover_scaling, channel_edit_scaling);
criterion::criterion_main!(interaction);
