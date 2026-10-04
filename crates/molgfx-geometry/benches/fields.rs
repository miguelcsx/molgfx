//! Source-load boundary reconstruction costs across grid sizes.

use criterion::{BenchmarkId, Criterion, Throughput};
use molgfx_core::{ScalarVolume, SegmentedVolume};
use molgfx_geometry::{extract_isosurface, extract_label_surfaces};
use molgfx_math::Mat4;
use std::{error::Error, hint::black_box, sync::Arc};

fn main() -> Result<(), Box<dyn Error>> {
    let mut criterion = Criterion::default().configure_from_args();
    let mut group = criterion.benchmark_group("field-boundaries");
    for side in [16_u16, 32, 64] {
        let dimensions = [u32::from(side); 3];
        let mut values = Vec::new();
        let mut labels = Vec::new();
        for _z in 0..side {
            for _y in 0..side {
                for x in 0..side {
                    values.push(f32::from(x));
                    labels.push(if x < side / 2 { 0 } else { u32::MAX });
                }
            }
        }
        let scalar = ScalarVolume::new(dimensions, Mat4::IDENTITY, Arc::from(values))?;
        let categorical = SegmentedVolume::new(dimensions, Mat4::IDENTITY, Arc::from(labels))?;
        group.throughput(Throughput::Elements(u64::from(side).pow(3)));
        group.bench_with_input(
            BenchmarkId::new("scalar", side),
            &scalar,
            |bencher, grid| {
                bencher
                    .iter(|| black_box(extract_isosurface(black_box(grid), f32::from(side) * 0.5)));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("categorical", side),
            &categorical,
            |bencher, grid| {
                bencher.iter(|| black_box(extract_label_surfaces(black_box(grid))));
            },
        );
    }
    group.finish();
    criterion.final_summary();
    Ok(())
}
