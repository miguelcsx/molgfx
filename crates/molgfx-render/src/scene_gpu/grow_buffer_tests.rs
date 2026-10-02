use super::grow_capacity;

#[test]
fn a_requirement_rounds_up_to_an_eighth_of_its_octave() {
    let Ok(capacity) = grow_capacity(1_000, 1 << 30, "test") else {
        panic!("a small requirement fits")
    };
    assert_eq!(capacity, 1_024);
    let Ok(capacity) = grow_capacity(17 << 20, 1 << 30, "test") else {
        panic!("a large requirement fits")
    };
    assert_eq!(capacity, 18 << 20, "17 MiB does not reserve 32 MiB");
}

#[test]
fn rounding_wastes_at_most_an_eighth_and_stays_four_byte_aligned() {
    for needed in [
        257_u64,
        1_000,
        4_097,
        65_537,
        (3 << 20) + 1,
        (100 << 20) + 7,
    ] {
        let Ok(capacity) = grow_capacity(needed, 1 << 40, "test") else {
            panic!("the requirement fits")
        };
        assert!(capacity >= needed, "{capacity} holds {needed}");
        assert!(
            capacity - needed <= needed / 8 + 1,
            "{capacity} for {needed}"
        );
        assert_eq!(capacity % 4, 0, "{capacity} is aligned");
    }
}

#[test]
fn a_tiny_requirement_still_reserves_the_minimum_allocation() {
    let Ok(capacity) = grow_capacity(4, 1 << 30, "test") else {
        panic!("a tiny requirement fits")
    };
    assert_eq!(capacity, 256);
}

#[test]
fn a_requirement_past_the_device_limit_is_refused() {
    assert!(grow_capacity(2_000, 1_000, "test").is_err());
}

#[test]
fn a_device_reporting_no_storage_is_refused_rather_than_divided_by() {
    assert!(grow_capacity(1, 0, "test").is_err());
}

#[test]
fn rounding_up_never_exceeds_the_device_limit() {
    let Ok(capacity) = grow_capacity(1_500, 2_000, "test") else {
        panic!("the requirement itself fits")
    };
    assert!((1_500..=2_000).contains(&capacity), "capacity {capacity}");
}
