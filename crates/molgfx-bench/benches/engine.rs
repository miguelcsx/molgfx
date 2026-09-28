//! Deterministic renderer-control benchmarks. These exercise scheduling policy,
//! not GPU work, so results never masquerade as device measurements.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use molgfx_render::{AdaptiveQuality, AdaptiveQualityConfig, QualityTier};

const BUDGET_NS: u64 = 16_666_666;

fn control_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine/control");
    for (name, samples) in [
        ("idle", vec![BUDGET_NS / 2; 512]),
        ("backpressure", vec![2 * BUDGET_NS; 128]),
        ("completed", vec![BUDGET_NS; 512]),
    ] {
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let mut quality =
                    AdaptiveQuality::new(AdaptiveQualityConfig::interactive(60), false);
                for sample in &samples {
                    black_box(quality.observe(*sample));
                }
                black_box(quality.tier())
            });
        });
    }
    group.finish();
}

fn representative_layers(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine/layer-combinations");
    for (name, layers) in [
        ("cartoon", 1_u32),
        ("cartoon+surface", 2),
        ("cartoon+surface+spacefill", 3),
        ("cartoon+surface+spacefill+labels", 4),
    ] {
        group.bench_function(BenchmarkId::new("scheduled_layers", name), |b| {
            b.iter(|| {
                let tier = match layers {
                    1 => QualityTier::Reduced,
                    2 => QualityTier::Standard,
                    3 => QualityTier::High,
                    _ => QualityTier::High,
                };
                black_box((layers, tier.temporal_samples(), tier.image_samples()))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, control_paths, representative_layers);
criterion_main!(benches);
