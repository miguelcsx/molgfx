use super::*;
use crate::time::TimeUnit;

fn frame(index: u64, time: f64, offset: f32) -> TrajectoryFrame {
    TrajectoryFrame::new(
        index,
        time,
        Arc::from([
            [offset, 0.0, 0.0],
            [offset + 1.0, 0.0, 0.0],
            [offset + 2.0, 0.0, 0.0],
        ]),
        "test:trajectory",
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn a_segment_retains_only_two_shared_frames_and_resolves_time() {
    let segment = TrajectorySegment::new(frame(4, 2.0, 0.0), frame(5, 4.0, 2.0), 2.5)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(segment.atom_count(), 3);
    assert!((segment.interpolation() - 0.25).abs() < f64::EPSILON);
    assert_eq!(segment.start().index(), 4);
    assert_eq!(segment.end().index(), 5);
}

#[test]
fn malformed_topology_time_and_coordinates_are_typed_errors() {
    let malformed = TrajectoryFrame::new(0, 0.0, Arc::from([[f32::NAN, 0.0, 0.0]]), "test");
    assert_eq!(
        malformed.err().map(|error| error.code()),
        Some("MOLGFX-E0036")
    );
    let segment = TrajectorySegment::new(frame(1, 1.0, 0.0), frame(2, 2.0, 1.0), 3.0);
    assert_eq!(
        segment.err().map(|error| error.code()),
        Some("MOLGFX-E0036")
    );
}

#[test]
fn a_frame_without_a_finite_time_is_a_typed_error() {
    let unrecorded = TrajectoryFrame::new(0, f64::NAN, Arc::from([[0.0_f32, 0.0, 0.0]]), "test");
    assert_eq!(
        unrecorded.err().map(|error| error.code()),
        Some("MOLGFX-E0036")
    );
}

#[test]
fn a_femtosecond_interval_at_a_microsecond_timestamp_keeps_its_fraction() {
    let base = SimTime::from_unit(1.0, TimeUnit::Microsecond)
        .expect("finite time")
        .seconds();
    let step = SimTime::from_unit(1.0, TimeUnit::Femtosecond)
        .expect("finite time")
        .seconds();
    let segment = TrajectorySegment::new(
        frame(0, base, 0.0),
        frame(1, base + step, 1.0),
        base + 0.25 * step,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        (segment.interpolation() - 0.25).abs() < 1.0e-3,
        "{}",
        segment.interpolation()
    );
}

#[test]
fn the_union_bound_contains_every_linear_sample() {
    let segment = TrajectorySegment::new(frame(0, 0.0, -4.0), frame(1, 1.0, 6.0), 0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    let bound = segment.union_aabb();
    assert!(bound.min.x <= -4.0);
    assert!(bound.max.x >= 8.0);
}

const TIMES: [f64; 5] = [0.0, 1.0, 2.5, 3.0, 5.0];
const VALUES: [f32; 5] = [0.0, 2.0, -1.0, 4.0, 1.0];

fn scalar_frame(index: usize) -> TrajectoryFrame {
    TrajectoryFrame::new(
        index as u64,
        TIMES[index],
        Arc::from([[VALUES[index], 0.0, 0.0]]),
        "test:hermite",
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn hermite_segment(start: usize, time: f64) -> TrajectorySegment {
    let before = start.checked_sub(1).map(scalar_frame);
    let after = (start + 2 < TIMES.len()).then(|| scalar_frame(start + 2));
    TrajectorySegment::new(scalar_frame(start), scalar_frame(start + 1), time)
        .and_then(|segment| segment.with_cubic_hermite(before, after))
        .unwrap_or_else(|error| panic!("{error}"))
}

fn position(segment: &TrajectorySegment) -> f64 {
    let weights = segment.blend_weights(segment.interpolation());
    let frames = [
        segment.before(),
        Some(segment.start()),
        Some(segment.end()),
        segment.after(),
    ];
    weights
        .iter()
        .zip(frames)
        .filter_map(|(weight, frame)| {
            frame.map(|frame| weight * f64::from(frame.positions()[0][0]))
        })
        .sum()
}

#[test]
fn hermite_velocity_matches_from_both_sides_of_every_interior_frame() {
    // One-sided finite differences over 1e-6 s; the O(eps) error stays far below 1e-4.
    let eps = 1.0e-6;
    for (boundary, time) in TIMES.iter().copied().enumerate().take(4).skip(1) {
        let before = hermite_segment(boundary - 1, time - eps);
        let ending = hermite_segment(boundary - 1, time);
        let starting = hermite_segment(boundary, time);
        let after = hermite_segment(boundary, time + eps);
        let left_velocity = (position(&ending) - position(&before)) / eps;
        let right_velocity = (position(&after) - position(&starting)) / eps;
        assert!(
            (left_velocity - right_velocity).abs() < 1.0e-4,
            "frame {boundary}: {left_velocity} vs {right_velocity}"
        );
    }
}

#[test]
fn hermite_reproduces_the_sample_frames_at_alpha_zero_and_one() {
    for start in 0..4 {
        let first = hermite_segment(start, TIMES[start]);
        let last = hermite_segment(start, TIMES[start + 1]);
        assert!((position(&first) - f64::from(VALUES[start])).abs() < 1.0e-12);
        assert!((position(&last) - f64::from(VALUES[start + 1])).abs() < 1.0e-12);
    }
}

#[test]
fn hermite_equals_linear_when_before_and_after_are_absent() {
    let linear = TrajectorySegment::new(scalar_frame(1), scalar_frame(2), 1.9)
        .unwrap_or_else(|error| panic!("{error}"));
    let hermite = linear
        .clone()
        .with_cubic_hermite(None, None)
        .unwrap_or_else(|error| panic!("{error}"));
    let linear_weights = linear.blend_weights(linear.interpolation());
    for (got, want) in hermite
        .blend_weights(hermite.interpolation())
        .iter()
        .zip(linear_weights)
    {
        assert!((got - want).abs() < 1.0e-12);
    }
}

#[test]
fn a_linear_segment_keeps_two_frames_and_hermite_keeps_four_only_when_selected() {
    let linear = TrajectorySegment::new(scalar_frame(1), scalar_frame(2), 1.5)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(linear.interpolation_kind(), InterpolationKind::Linear);
    assert_eq!(linear.resident_frames(), 2);
    assert!(linear.before().is_none() && linear.after().is_none());
    let hermite = hermite_segment(1, 1.5);
    assert_eq!(
        hermite.interpolation_kind(),
        InterpolationKind::CubicHermite
    );
    assert_eq!(hermite.resident_frames(), 4);
    assert_eq!(hermite_segment(0, 0.5).resident_frames(), 3);
}

#[test]
fn hermite_neighbours_must_bracket_the_segment_and_match_its_atom_count() {
    let segment = || {
        TrajectorySegment::new(scalar_frame(1), scalar_frame(2), 1.5)
            .unwrap_or_else(|error| panic!("{error}"))
    };
    let swapped = segment().with_cubic_hermite(Some(scalar_frame(3)), None);
    assert_eq!(
        swapped.err().map(|error| error.code()),
        Some("MOLGFX-E0036")
    );
    let wide = TrajectoryFrame::new(0, 0.0, Arc::from([[0.0_f32; 3], [1.0; 3]]), "wide")
        .unwrap_or_else(|error| panic!("{error}"));
    let mismatched = segment().with_cubic_hermite(Some(wide), None);
    assert_eq!(
        mismatched.err().map(|error| error.code()),
        Some("MOLGFX-E0036")
    );
}

#[test]
fn the_hermite_bound_contains_the_overshoot_of_the_spline() {
    let segment = hermite_segment(1, 1.5);
    let bound = segment.union_aabb();
    for step in 0..=100 {
        let mut sampled = segment.clone();
        sampled
            .set_sample_time(1.0 + 1.5 * f64::from(step) / 100.0)
            .unwrap_or_else(|error| panic!("{error}"));
        let x = crate::narrow_to_f32(position(&sampled)).expect("finite");
        assert!(
            x >= bound.min.x - 1.0e-4 && x <= bound.max.x + 1.0e-4,
            "{x}"
        );
    }
}
