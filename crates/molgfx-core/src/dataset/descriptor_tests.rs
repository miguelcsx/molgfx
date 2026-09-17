use super::*;

#[test]
fn a_span_maps_rows_above_u32_to_local_indices() {
    let first = LogicalRow::new(u64::from(u32::MAX) + 100);
    let span = match ChunkSpan::new(first, 8) {
        Ok(span) => span,
        Err(error) => panic!("valid span rejected: {error}"),
    };
    let chunk = ChunkId::new(9);
    let local = match span.local_row(chunk, LogicalRow::new(first.get() + 7)) {
        Ok(local) => local,
        Err(error) => panic!("contained row rejected: {error}"),
    };
    assert_eq!(local.get(), 7);
    assert_eq!(
        span.logical_row(chunk, local),
        Ok(LogicalRow::new(first.get() + 7))
    );
}

#[test]
fn a_span_rejects_rows_from_another_chunk() {
    let span = match ChunkSpan::new(LogicalRow::new(100), 4) {
        Ok(span) => span,
        Err(error) => panic!("valid span rejected: {error}"),
    };
    assert!(matches!(
        span.local_row(ChunkId::new(2), LogicalRow::new(99)),
        Err(DatasetError::LogicalRowOutsideChunk { .. })
    ));
    assert!(matches!(
        span.logical_row(ChunkId::new(2), LocalRow::new(4)),
        Err(DatasetError::LogicalRowOutsideChunk { .. })
    ));
}

#[test]
fn bounds_reject_non_finite_or_inverted_axes() {
    assert_eq!(
        ChunkBounds::new([0.0; 3], [f32::NAN, 1.0, 1.0]),
        Err(DatasetError::InvalidBounds)
    );
    assert_eq!(
        ChunkBounds::new([2.0, 0.0, 0.0], [1.0; 3]),
        Err(DatasetError::InvalidBounds)
    );
}
