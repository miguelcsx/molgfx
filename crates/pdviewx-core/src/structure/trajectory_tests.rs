use super::*;

fn frame(index: u64, time: f32, offset: f32) -> TrajectoryFrame {
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
    assert!((segment.interpolation() - 0.25).abs() < f32::EPSILON);
    assert_eq!(segment.start().index(), 4);
    assert_eq!(segment.end().index(), 5);
}

#[test]
fn malformed_topology_time_and_coordinates_are_typed_errors() {
    let malformed = TrajectoryFrame::new(0, 0.0, Arc::from([[f32::NAN, 0.0, 0.0]]), "test");
    assert_eq!(
        malformed.err().map(|error| error.code()),
        Some("PDVIEWX-E0036")
    );
    let segment = TrajectorySegment::new(frame(1, 1.0, 0.0), frame(2, 2.0, 1.0), 3.0);
    assert_eq!(
        segment.err().map(|error| error.code()),
        Some("PDVIEWX-E0036")
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
