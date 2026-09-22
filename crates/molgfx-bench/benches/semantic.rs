//! Cost of the semantic layer's everyday operations.
//!
//! These are the operations an interactive application performs between
//! frames — moving a slider, hovering an atom, toggling a representation — and
//! the ones a declarative API most easily makes accidentally expensive. Each
//! group is written so that its cost either is, or visibly is not, independent
//! of scene size.
//!
//! Input is generated in memory, so every group runs on any checkout rather
//! than skipping when no scene corpus is present.

use criterion::{BenchmarkId, Criterion, Throughput};
use molgfx::{Scene, rep, sel, visual};
use molgfx_bench::synthetic;
use std::hint::black_box;

/// Residue counts spanning a ligand, a domain and a small complex.
const SIZES: [usize; 3] = [64, 1_024, 8_192];

fn scene_of(residues: usize) -> Scene {
    let structure = synthetic::structure(residues);
    match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("synthetic scene must resolve: {error}"),
    }
}

fn represented(residues: usize, representations: usize) -> Scene {
    let mut scene = scene_of(residues);
    for _ in 0..representations {
        if let Err(error) = scene.add(rep::cartoon(sel::all())) {
            panic!("representation must add: {error}")
        }
    }
    scene
}

fn first_representation(scene: &Scene) -> molgfx::RepresentationId {
    match scene.spec().representations.keys().next().copied() {
        Some(id) => id,
        None => panic!("the scene has a representation"),
    }
}

/// Building a scene is inherently proportional to the molecule; this records
/// the constant so a regression in it is visible.
fn construction(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("scene/construction");
    for residues in SIZES {
        let structure = synthetic::structure(residues);
        group.throughput(Throughput::Elements(residues as u64));
        let _ = group.bench_with_input(
            BenchmarkId::from_parameter(residues),
            &structure,
            |bencher, structure| {
                bencher.iter(|| black_box(Scene::from_structure(black_box(structure))));
            },
        );
    }
    group.finish();
}

/// An appearance edit must not scale with the molecule. If these three lines
/// diverge across sizes, an edit is rebuilding molecular records.
fn appearance_edits(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("edit/opacity");
    for residues in SIZES {
        let mut scene = represented(residues, 1);
        let id = first_representation(&scene);
        let mut opacity = 0.0_f32;
        let _ = group.bench_function(BenchmarkId::from_parameter(residues), |bencher| {
            bencher.iter(|| {
                opacity = if opacity > 0.9 { 0.1 } else { opacity + 0.01 };
                match scene.set_opacity(id, opacity) {
                    Ok(()) => {}
                    Err(error) => panic!("opacity edit must apply: {error}"),
                }
            });
        });
    }
    group.finish();
}

/// A visibility toggle must reduce to a flag, never to an eviction.
fn visibility_edits(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("edit/visibility");
    for residues in SIZES {
        let mut scene = represented(residues, 1);
        let id = first_representation(&scene);
        let mut visible = true;
        let _ = group.bench_function(BenchmarkId::from_parameter(residues), |bencher| {
            bencher.iter(|| {
                visible = !visible;
                match scene.set_visible(id, visible) {
                    Ok(()) => {}
                    Err(error) => panic!("visibility edit must apply: {error}"),
                }
            });
        });
    }
    group.finish();
}

/// A parameter update resolves to one slot write; it must not recompile the
/// program it belongs to.
fn parameter_edits(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("edit/parameter");
    for residues in SIZES {
        let mut scene = scene_of(residues);
        let parameter = visual::Parameter::new("opacity_scale", 0.5_f32);
        let style = visual::VisualStyle {
            color: visual::ColorExpr::Constant(molgfx::Color::rgb(200, 200, 200)),
            opacity: parameter.clone().into(),
            visible: visual::BoolExpr::Constant(true),
        };
        if let Err(error) = scene.add(rep::cartoon(sel::all()).visual(style)) {
            panic!("styled representation must add: {error}")
        }
        let id = first_representation(&scene);
        let mut value = 0.0_f32;
        let _ = group.bench_function(BenchmarkId::from_parameter(residues), |bencher| {
            bencher.iter(|| {
                value = if value > 0.9 { 0.1 } else { value + 0.01 };
                match scene.set_parameter(id, &parameter, value) {
                    Ok(()) => {}
                    Err(error) => panic!("parameter edit must apply: {error}"),
                }
            });
        });
    }
    group.finish();
}

/// An interaction edit re-evaluates one molecular query and rewrites state
/// bits. It is proportional to the molecule by nature, and must not do more.
fn interaction_edits(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("edit/interaction");
    for residues in SIZES {
        let mut scene = represented(residues, 1);
        group.throughput(Throughput::Elements(residues as u64));
        let mut selected = false;
        let _ = group.bench_function(BenchmarkId::from_parameter(residues), |bencher| {
            bencher.iter(|| {
                selected = !selected;
                let target = if selected {
                    Some(sel::all().into())
                } else {
                    None
                };
                match scene.set_interaction(molgfx::InteractionChannel::Selected, target) {
                    Ok(()) => {}
                    Err(error) => panic!("interaction edit must apply: {error}"),
                }
            });
        });
    }
    group.finish();
}

/// An edit must cost the same whether the scene holds one representation or
/// many: only the touched one should be revalidated.
fn edit_cost_by_representation_count(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("edit/opacity_by_representation_count");
    for count in [1_usize, 4, 16, 64] {
        let mut scene = represented(256, count);
        let id = first_representation(&scene);
        let mut opacity = 0.0_f32;
        let _ = group.bench_function(BenchmarkId::from_parameter(count), |bencher| {
            bencher.iter(|| {
                opacity = if opacity > 0.9 { 0.1 } else { opacity + 0.01 };
                match scene.set_opacity(id, opacity) {
                    Ok(()) => {}
                    Err(error) => panic!("opacity edit must apply: {error}"),
                }
            });
        });
    }
    group.finish();
}

/// Lowering a visual program must be linear in its node count. Keying
/// subexpressions by their serialized form made this quadratic.
fn visual_lowering_by_node_count(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("visual/compile_by_node_count");
    for nodes in [8_usize, 32, 64, 120] {
        let mut expression = visual::ScalarExpr::input("time");
        for _ in 0..nodes {
            expression = expression * visual::ScalarExpr::from(0.999);
        }
        let style = visual::VisualStyle {
            color: visual::ColorExpr::Constant(molgfx::Color::rgb(10, 20, 30)),
            opacity: expression,
            visible: visual::BoolExpr::Constant(true),
        };
        group.throughput(Throughput::Elements(nodes as u64));
        let _ = group.bench_function(BenchmarkId::from_parameter(nodes), |bencher| {
            bencher.iter(|| black_box(black_box(&style).compile()));
        });
    }
    group.finish();
}

/// Reading a specification parses and validates it once, not once per layer.
fn specification_parsing(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("spec/from_json");
    for count in [1_usize, 16, 64] {
        let scene = represented(64, count);
        let json = match scene.spec().to_json() {
            Ok(json) => json,
            Err(error) => panic!("specification must serialize: {error}"),
        };
        group.throughput(Throughput::Bytes(json.len() as u64));
        let _ = group.bench_function(BenchmarkId::from_parameter(count), |bencher| {
            bencher.iter(|| black_box(molgfx::SceneSpec::from_json(black_box(&json))));
        });
    }
    group.finish();
}

criterion::criterion_group!(
    semantic,
    construction,
    appearance_edits,
    visibility_edits,
    parameter_edits,
    interaction_edits,
    edit_cost_by_representation_count,
    visual_lowering_by_node_count,
    specification_parsing,
);
criterion::criterion_main!(semantic);
