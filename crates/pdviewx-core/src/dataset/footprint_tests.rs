use super::*;

#[test]
fn footprint_totals_use_checked_arithmetic() {
    let footprint = ChunkFootprint::new(1, 2, 3, 4);
    assert_eq!(footprint.total_bytes(), Ok(10));
}

#[test]
fn footprint_overflow_is_a_typed_error() {
    let footprint = ChunkFootprint::new(u64::MAX, 1, 0, 0);
    assert_eq!(
        footprint.total_bytes(),
        Err(DatasetError::FootprintOverflow)
    );
}
