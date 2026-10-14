use super::*;
use crate::TimeUnit;

fn assert_close(actual: f64, expected: f64) {
    let scale = expected.abs().max(f64::MIN_POSITIVE);
    assert!(
        (actual - expected).abs() <= 1e-12 * scale,
        "{actual} != {expected}"
    );
}

fn femtoseconds(value: f64) -> SimTime {
    SimTime::from_unit(value, TimeUnit::Femtosecond).expect("finite time")
}

#[test]
fn dilation_advances_physical_time_at_the_stated_pace_and_holds_its_end() {
    let pace = SimTime::from_unit(1.0, TimeUnit::Nanosecond).expect("finite time");
    let range = [
        SimTime::from_unit(0.0, TimeUnit::Nanosecond).expect("finite time"),
        SimTime::from_unit(50.0, TimeUnit::Nanosecond).expect("finite time"),
    ];
    let warp = TimeWarp::dilation(pace, range, PlaybackMode::Clamp).expect("dilation");
    assert_close(warp.sample(0.0).expect("start"), 0.0);
    assert_close(warp.sample(3.0).expect("three seconds"), 3e-9);
    assert_close(warp.sample(60.0).expect("held"), 50e-9);
}

#[test]
fn fit_plays_a_physical_interval_once_across_the_playback_duration() {
    let vibration = [femtoseconds(0.0), femtoseconds(100.0)];
    let warp = TimeWarp::fit(vibration, 10.0, PlaybackMode::Clamp).expect("fit");
    assert_close(warp.sample(0.0).expect("start"), 0.0);
    assert_close(warp.sample(2.5).expect("quarter"), 25e-15);
    assert_close(warp.sample(10.0).expect("end"), 100e-15);
    assert_close(warp.sample(30.0).expect("held"), 100e-15);
    assert_close(warp.phase(2.5).expect("phase"), 0.25);
}

#[test]
fn fitted_loop_and_ping_pong_wrap_the_interval_after_it_has_played_once() {
    let vibration = [femtoseconds(0.0), femtoseconds(100.0)];
    let looped = TimeWarp::fit(vibration, 10.0, PlaybackMode::Loop).expect("loop");
    assert_close(looped.sample(12.5).expect("wrapped"), 25e-15);
    let ping_pong = TimeWarp::fit(vibration, 10.0, PlaybackMode::PingPong).expect("ping-pong");
    assert_close(ping_pong.sample(12.5).expect("reflected"), 75e-15);
}

#[test]
fn fit_rejects_a_playback_duration_that_is_not_finite_and_positive() {
    let interval = [femtoseconds(0.0), femtoseconds(100.0)];
    for duration in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            TimeWarp::fit(interval, duration, PlaybackMode::Clamp).is_err(),
            "duration {duration} must be rejected"
        );
    }
}

#[test]
fn fit_rejects_an_empty_or_reversed_physical_interval() {
    let reversed = [femtoseconds(100.0), femtoseconds(0.0)];
    assert!(TimeWarp::fit(reversed, 10.0, PlaybackMode::Clamp).is_err());
    let empty = [femtoseconds(5.0), femtoseconds(5.0)];
    assert!(TimeWarp::fit(empty, 10.0, PlaybackMode::Clamp).is_err());
}

#[test]
fn a_fifty_nanosecond_interval_fitted_to_ten_seconds_reads_twenty_five_at_five() {
    let range = [
        SimTime::from_unit(0.0, TimeUnit::Nanosecond).expect("finite time"),
        SimTime::from_unit(50.0, TimeUnit::Nanosecond).expect("finite time"),
    ];
    let warp = TimeWarp::fit(range, 10.0, PlaybackMode::Clamp).expect("fit");
    assert_close(warp.sample(5.0).expect("midpoint"), 25e-9);
}
