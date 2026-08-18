use super::*;

#[test]
fn surface_field_dispatch_stays_within_the_portable_dimension_limit() {
    assert_eq!(dispatch_grid(1), [1, 1]);
    assert_eq!(dispatch_grid(64 * 65_535), [65_535, 1]);
    assert_eq!(dispatch_grid(64 * 65_535 + 1), [65_535, 2]);
    let grid = dispatch_grid(192 * 192 * 192);
    assert_eq!(grid, [65_535, 2]);
    assert!(grid.into_iter().all(|value| value <= 65_535));
}
