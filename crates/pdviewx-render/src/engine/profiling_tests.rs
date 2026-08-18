use super::*;

#[test]
fn same_submission_timestamps_use_the_current_start() {
    assert_eq!(timestamp_delta(1_000, 1_750, Some(100)), 750);
}

#[test]
fn delayed_pass_end_timestamps_pair_with_the_previous_start() {
    assert_eq!(timestamp_delta(2_000, 1_750, Some(1_000)), 750);
}

#[test]
fn the_first_delayed_sample_is_an_explicit_zero_warmup() {
    assert_eq!(timestamp_delta(2_000, 1_750, None), 0);
}
