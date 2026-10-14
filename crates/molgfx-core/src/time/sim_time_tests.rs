use super::*;

/// Asserts agreement to twelve significant digits, the precision a double keeps
/// through one unit conversion.
fn assert_close(actual: f64, expected: f64) {
    let tolerance = expected.abs() * 1.0e-12;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} is not {expected}"
    );
}

#[test]
fn a_bare_number_reads_as_canonical_seconds() {
    assert_close(SimTime::from(2.5).seconds(), 2.5);
}

#[test]
fn a_value_survives_a_round_trip_through_every_unit() {
    for unit in TimeUnit::ALL {
        assert_close(
            SimTime::from_unit(12.5, unit)
                .expect("finite time")
                .to_unit(unit),
            12.5,
        );
    }
}

#[test]
fn a_picosecond_value_reads_as_the_same_time_in_nanoseconds() {
    let time = SimTime::from_unit(2500.0, TimeUnit::Picosecond).expect("finite time");
    assert_close(time.to_unit(TimeUnit::Nanosecond), 2.5);
}

#[test]
fn a_femtosecond_offset_survives_a_millisecond_timestamp() {
    let base = SimTime::from_unit(1.0, TimeUnit::Millisecond)
        .expect("finite time")
        .seconds();
    let offset = base
        + SimTime::from_unit(1.0, TimeUnit::Femtosecond)
            .expect("finite time")
            .seconds();
    let recovered = (offset - base) / TimeUnit::Femtosecond.seconds();
    assert!((recovered - 1.0).abs() < 1.0e-3, "{recovered}");
}

#[test]
fn times_order_by_their_physical_length_regardless_of_unit() {
    let ten_picoseconds = SimTime::from_unit(10.0, TimeUnit::Picosecond).expect("finite time");
    let one_nanosecond = SimTime::from_unit(1.0, TimeUnit::Nanosecond).expect("finite time");
    assert!(ten_picoseconds < one_nanosecond);
}

#[test]
fn display_writes_a_time_in_the_largest_unit_it_reaches() {
    assert_eq!(
        SimTime::from_unit(12.4, TimeUnit::Nanosecond)
            .expect("finite time")
            .to_string(),
        "12.4 ns"
    );
    assert_eq!(
        SimTime::from_unit(1.0, TimeUnit::Millisecond)
            .expect("finite time")
            .to_string(),
        "1 ms"
    );
    assert_eq!(
        SimTime::from_unit(3.0, TimeUnit::Microsecond)
            .expect("finite time")
            .to_string(),
        "3 µs"
    );
    assert_eq!(
        SimTime::from_unit(250.0, TimeUnit::Femtosecond)
            .expect("finite time")
            .to_string(),
        "250 fs"
    );
    assert_eq!(SimTime::from(1.5).to_string(), "1.5 s");
}

#[test]
fn display_writes_zero_in_seconds() {
    assert_eq!(SimTime::from(0.0).to_string(), "0 s");
}

#[test]
fn nan_and_infinite_seconds_are_rejected_at_construction() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(SimTime::new(bad).is_err(), "{bad} must be rejected");
        assert!(SimTime::from_unit(bad, TimeUnit::Nanosecond).is_err());
    }
}

#[test]
fn a_finite_value_that_overflows_when_scaled_is_rejected() {
    assert!(SimTime::from_unit(f64::MAX, TimeUnit::Second).is_ok());
    assert!(SimTime::new(f64::MAX * 2.0).is_err());
}

#[test]
fn a_unit_round_trip_holds_from_attoseconds_to_seconds() {
    for unit in TimeUnit::ALL {
        assert_close(
            SimTime::from_unit(3.25, unit)
                .expect("finite time")
                .to_unit(unit),
            3.25,
        );
    }
}

#[test]
fn ordering_is_strict_and_an_unordered_pair_is_never_earlier() {
    let early = SimTime::from(1.0);
    let late = SimTime::from(2.0);
    assert!(early.is_before(late));
    assert!(!late.is_before(early));
    assert!(!early.is_before(early));
    let unrecorded = SimTime::from(f64::NAN);
    assert!(!early.is_before(unrecorded));
    assert!(!unrecorded.is_before(early));
}
