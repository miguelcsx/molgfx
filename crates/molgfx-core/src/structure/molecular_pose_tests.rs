use super::*;
use crate::{DatasetId, StructureAsset, TrajectoryFrame};
use std::sync::Arc;

fn placed() -> PlacedStructure {
    let (source, _) = molframe::read_bytes(
        b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n"
            .to_vec(),
        Some("pose.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    PlacedStructure::from_asset(&StructureAsset::new(DatasetId::LEGACY, &source).expect("asset"))
}

fn segment(start: Arc<[[f32; 3]]>, end: Arc<[[f32; 3]]>) -> TrajectorySegment {
    TrajectorySegment::new(
        TrajectoryFrame::new(0, 0.0, start, "test").expect("start"),
        TrajectoryFrame::new(1, 1.0, end, "test").expect("end"),
        0.25,
    )
    .expect("segment")
}

#[test]
fn captured_samples_borrow_frames_and_reject_changed_time_or_independently_owned_frame_storage() {
    let mut placed = placed();
    let original = MolecularPose::capture(&placed);
    assert_eq!(
        original.positions().as_ptr(),
        placed.source.coordinates().as_ptr()
    );
    assert!(original.matches(&placed));
    let start: Arc<[[f32; 3]]> = Arc::from([[2.0, 0.0, 0.0]]);
    let end: Arc<[[f32; 3]]> = Arc::from([[6.0, 0.0, 0.0]]);
    placed
        .replace_trajectory(segment(start.clone(), end.clone()))
        .expect("bind");
    let pose = MolecularPose::capture(&placed);
    assert!(!original.matches(&placed));
    assert_eq!(pose.positions().as_ptr(), start.as_ptr());
    assert_eq!(pose.end_positions().expect("end").as_ptr(), end.as_ptr());
    assert_eq!(
        pose.interpolation().expect("weight").to_bits(),
        0.25_f32.to_bits()
    );
    assert!(pose.matches(&placed));
    placed.set_trajectory_time(0.5).expect("advance");
    assert!(!pose.matches(&placed));
    assert!(pose.matches_interval(&placed));
    placed.set_trajectory_time(0.25).expect("reverse");
    assert!(pose.matches(&placed));
    placed
        .replace_trajectory(segment(Arc::from([[2.0, 0.0, 0.0]]), end))
        .expect("other storage");
    assert!(
        !pose.matches(&placed),
        "identical coordinates do not mean identical retained input"
    );
    assert!(!pose.matches_interval(&placed));
    placed.clear_trajectory();
    assert!(original.matches(&placed));
    assert!(!pose.matches(&placed));
    assert_eq!(
        pose.positions().as_ptr(),
        start.as_ptr(),
        "worker capture retains input after playback stops"
    );
}

#[test]
fn reconstructed_samples_share_validated_frames_and_reject_a_different_topology_length() {
    let placed = placed();
    let interval = segment(Arc::from([[0.0; 3]]), Arc::from([[2.0, 0.0, 0.0]]));
    let pose =
        MolecularPose::new(placed.source.clone(), Some(interval.clone())).expect("reconstruct");
    assert_eq!(
        pose.positions().as_ptr(),
        interval.start().positions().as_ptr()
    );
    assert_eq!(
        pose.trajectory()
            .expect("interval")
            .sample_seconds()
            .to_bits(),
        0.25_f32.to_bits()
    );
    let wrong = segment(Arc::from([[0.0; 3]; 2]), Arc::from([[2.0, 0.0, 0.0]; 2]));
    assert!(MolecularPose::new(placed.source.clone(), Some(wrong)).is_err());
    let static_pose = MolecularPose::new(placed.source.clone(), None).expect("static");
    assert!(static_pose.matches(&placed));
}
