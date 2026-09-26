//! The authoring command path: parsing, planning and applying edits.
//!
//! Each edit is measured end to end — text to applied scene revision — on
//! generated chains of 5k, 100k and 1M atoms, so the cost of a colour change
//! that only rewrites a uniform can be told apart from one that re-evaluates a
//! query or rebuilds geometry. Input is generated in memory, so this runs on
//! any checkout.

use criterion::{BenchmarkId, Criterion, Throughput};
use molgfx::Scene;
use molgfx::command::{Program, Session};
use molgfx_bench::synthetic;
use std::hint::black_box;

/// Residue counts giving 5k, 100k and 1M atoms.
const SIZES: [usize; 3] = [1_000, 20_000, 200_000];

/// Atoms per generated residue, matching `synthetic::ATOMS`.
const ATOMS_PER_RESIDUE: u64 = 5;

/// A small program touching every kind of statement.
const PROGRAM: &str = "\
select core, resid 1:40 and name CA CB
select site, byres (within 6 of $core)
show cartoon width=1.5 as main, all
show ball_and_stick radius=0.3 as site, $site
color red, $core
opacity 0.6, @main
focus $site";

fn atoms(residues: usize) -> u64 {
    residues as u64 * ATOMS_PER_RESIDUE
}

/// A scene with a layer drawing everything and one depending on `site`.
fn prepared(residues: usize) -> (Scene, Session) {
    let structure = synthetic::structure(residues);
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("synthetic scene must resolve: {error}"),
    };
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select site, resid 1:10; show spacefill as all_atoms, all; show ball_and_stick as site, $site; color yellow, $site",
    );
    (scene, session)
}

fn run(session: &mut Session, scene: &mut Scene, source: &str) {
    if let Err(errors) = session.execute_text(scene, source) {
        panic!("{source:?} must run: {}", errors.render(source));
    }
}

fn parsed(source: &str) -> Program {
    match Program::parse(source) {
        Ok(program) => program,
        Err(errors) => panic!("{source:?} must parse: {errors}"),
    }
}

/// Parsing alone: splitting, argument checking and `MolFrame` compilation.
fn parsing(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("command/parse");
    let _ = group.bench_function("program", |bencher| {
        bencher.iter(|| black_box(Program::parse(black_box(PROGRAM)).is_ok()));
    });
    let _ = group.bench_function("single", |bencher| {
        bencher.iter(|| black_box(Program::parse(black_box("color red, @main")).is_ok()));
    });
    group.finish();
}

/// Edits across sizes, alternating between two states so every iteration
/// changes the scene.
fn edits(criterion: &mut Criterion) {
    let cases: [(&str, [&str; 2]); 5] = [
        (
            "layer_color",
            ["color red, @all_atoms", "color blue, @all_atoms"],
        ),
        ("visibility", ["hide @all_atoms", "show @all_atoms"]),
        (
            "rule_color",
            ["color red, resid 1:50", "color blue, resid 1:50"],
        ),
        (
            "redefinition",
            ["select site, resid 11:20", "select site, resid 1:10"],
        ),
        (
            "structural",
            ["show lines as extra, resid 1:100", "remove @extra"],
        ),
    ];
    for residues in SIZES {
        let (mut scene, mut session) = prepared(residues);
        let mut group = criterion.benchmark_group("command/edit");
        group.throughput(Throughput::Elements(atoms(residues)));
        group.sample_size(if residues >= 100_000 { 10 } else { 30 });
        for (name, [forward, back]) in cases {
            let (forward, back) = (parsed(forward), parsed(back));
            let mut flip = false;
            let _ = group.bench_function(BenchmarkId::new(name, atoms(residues)), |bencher| {
                bencher.iter(|| {
                    flip = !flip;
                    let program = if flip { &forward } else { &back };
                    black_box(session.execute(&mut scene, program).is_ok())
                });
            });
        }
        let (undo, redo) = (parsed("undo"), parsed("redo"));
        run(&mut session, &mut scene, "color green, resid 5:15");
        let mut flip = false;
        let _ = group.bench_function(BenchmarkId::new("undo_redo", atoms(residues)), |bencher| {
            bencher.iter(|| {
                flip = !flip;
                let program = if flip { &undo } else { &redo };
                black_box(session.execute(&mut scene, program).is_ok())
            });
        });
        group.finish();
    }
}

/// The encoded size of each edit's patch, which is what crosses to a viewer.
fn patch_sizes(criterion: &mut Criterion) {
    let (mut scene, mut session) = prepared(SIZES[0]);
    let mut group = criterion.benchmark_group("command/patch_json");
    for (name, source) in [
        ("layer_color", "color red, @all_atoms"),
        ("redefinition", "select site, resid 11:20"),
        ("structural", "show lines as extra, resid 1:100"),
    ] {
        let outcome = match session.execute_text(&mut scene, source) {
            Ok(outcome) => outcome,
            Err(errors) => panic!("{errors}"),
        };
        let Some(patch) = outcome.patch else {
            continue;
        };
        let bytes = patch.to_json().map_or(0, |json| json.len());
        eprintln!("command/patch_json/{name}: {bytes} bytes");
        let _ = group.bench_function(name, |bencher| {
            bencher.iter(|| black_box(patch.to_json().is_ok()));
        });
    }
    group.finish();
}

criterion::criterion_group!(command, parsing, edits, patch_sizes);
criterion::criterion_main!(command);
