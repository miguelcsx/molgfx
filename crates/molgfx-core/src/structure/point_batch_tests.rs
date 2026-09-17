use super::{PointBatch, PointGlyph, PointStyle};
use crate::{SourceNamespace, SourceRows};
use std::sync::Arc;

#[test]
fn point_positions_remain_twelve_bytes_and_bounds_are_thread_deterministic() {
    let positions: Arc<[[f32; 3]]> = (0_u16..40_000)
        .map(|row| [f32::from(row) * 0.001, f32::from(row % 17), -2.0])
        .collect::<Vec<_>>()
        .into();
    let build = || {
        PointBatch::new(
            Arc::clone(&positions),
            SourceRows::ordered(SourceNamespace(1), u32::try_from(positions.len()).unwrap()),
            PointGlyph::Disc,
            PointStyle::default(),
        )
        .unwrap()
        .bounds()
    };
    let expected = build();
    for threads in [1, 2, 3, 8, 16] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        assert_eq!(pool.install(build), expected);
    }
    assert_eq!(std::mem::size_of_val(&positions[0]), 12);
}
