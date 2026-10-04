use super::*;

#[test]
fn a_sweep_rejects_unaddressable_vertices_before_emitting_any_triangles() {
    assert_eq!(
        check_sweep(u32::MAX as usize - 65, 0, 2, 96, false),
        Err(PackingError::IndexOverflow {
            resource: "ribbon vertices",
            index: u64::from(u32::MAX) + 1,
        })
    );
    assert!(check_sweep(u32::MAX as usize - 66, 0, 2, 96, false).is_ok());
}

#[test]
fn a_sweep_rejects_an_index_count_that_native_indirect_draw_cannot_represent() {
    assert_eq!(
        check_sweep(0, u32::MAX as usize - 191, 2, 96, false),
        Err(PackingError::IndexOverflow {
            resource: "ribbon draw indices",
            index: u64::from(u32::MAX) + 1,
        })
    );
    assert!(check_sweep(0, u32::MAX as usize - 192, 2, 96, false).is_ok());
}

#[test]
fn flat_shells_and_caps_use_their_actual_counts_at_the_draw_limit() {
    assert!(check_sweep(0, u32::MAX as usize - 36, 2, 24, true).is_ok());
    assert!(check_sweep(0, u32::MAX as usize - 35, 2, 24, true).is_err());
}

#[test]
fn arithmetic_overflow_is_reported_instead_of_wrapping_a_valid_draw() {
    assert!(check_sweep(0, 0, usize::MAX, 0, false).is_err());
    assert!(check_sweep(0, 0, 2, u64::MAX, false).is_err());
}
