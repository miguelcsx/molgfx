//! Criterion coverage for the CPU math kernels: BVH build/rebuild/query,
//! Catmull-Rom sampling and parallel-transport frames. Inputs are synthetic
//! but deterministic, so runs are comparable across machines and flamegraphs.

use criterion::{Criterion, Throughput};
use molgfx_math::{
    Aabb, Bvh, BvhBuildScratch, CurveSample, Vec3, parallel_transport, sample_catmull_rom,
};
use std::hint::black_box;

/// A deterministic double-helix point cloud — spatially coherent like a real
/// backbone, so BVH traversal exercises realistic overlap, not a uniform grid.
/// The parameter accumulates as a float, sidestepping an index-to-`f32` cast.
fn helix(n: usize) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(n);
    let mut t = 0.0f32;
    for i in 0..n {
        let strand = if i % 2 == 0 {
            0.0
        } else {
            std::f32::consts::PI
        };
        points.push(Vec3::new(
            (t + strand).cos() * 5.0,
            t * 0.6,
            (t + strand).sin() * 5.0,
        ));
        t += 0.35;
    }
    points
}

fn bounds_of(points: &[Vec3]) -> Vec<Aabb> {
    let r = Vec3::splat(1.7);
    points.iter().map(|&p| Aabb::new(p - r, p + r)).collect()
}

fn bench_bvh(c: &mut Criterion) {
    for &n in &[1_000usize, 10_000, 50_000] {
        let points = helix(n);
        let bounds = bounds_of(&points);
        let mut group = c.benchmark_group("bvh");
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(format!("build_{n}"), |b| {
            b.iter(|| black_box(built_bvh(black_box(&bounds))));
        });

        // The allocation-free trajectory path: storage and scratch are reused.
        group.bench_function(format!("rebuild_{n}"), |b| {
            let mut bvh = built_bvh(&bounds);
            let mut scratch = BvhBuildScratch::default();
            b.iter(|| match bvh.rebuild(black_box(&bounds), &mut scratch) {
                Ok(()) => black_box(()),
                Err(error) => panic!("{error}"),
            });
        });

        // Radius-3 sphere queries at every point, reusing traversal scratch.
        group.bench_function(format!("sphere_candidates_{n}"), |b| {
            let bvh = built_bvh(&bounds);
            let (mut traversal, mut out) = (Vec::new(), Vec::new());
            b.iter(|| {
                let mut hits = 0usize;
                for &p in &points {
                    bvh.sphere_candidates(black_box(p), 3.0, &mut traversal, &mut out);
                    hits += out.len();
                }
                black_box(hits)
            });
        });
        group.finish();
    }
}

fn built_bvh(bounds: &[Aabb]) -> Bvh {
    match Bvh::build(bounds) {
        Ok(hierarchy) => hierarchy,
        Err(error) => panic!("{error}"),
    }
}

fn bench_curves(c: &mut Criterion) {
    for &n in &[256usize, 4_096] {
        let control = helix(n);
        let mut group = c.benchmark_group("curves");
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(format!("sample_catmull_rom_{n}"), |b| {
            let mut out = Vec::new();
            b.iter(|| sample_catmull_rom(black_box(&control), 0.1, 16, &mut out));
        });

        let mut samples: Vec<CurveSample> = Vec::new();
        sample_catmull_rom(&control, 0.1, 16, &mut samples);
        group.bench_function(format!("parallel_transport_{}", samples.len()), |b| {
            let mut frames = Vec::new();
            b.iter(|| parallel_transport(black_box(&samples), &mut frames));
        });
        group.finish();
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    bench_bvh(&mut criterion);
    bench_curves(&mut criterion);
    criterion.final_summary();
}
