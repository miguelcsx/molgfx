use super::grow_capacity;

#[test]
fn a_requirement_rounds_up_to_the_next_power_of_two() {
    let Ok(capacity) = grow_capacity(1_000, 1 << 30, "test") else {
        panic!("a small requirement fits")
    };
    assert_eq!(capacity, 1_024);
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
