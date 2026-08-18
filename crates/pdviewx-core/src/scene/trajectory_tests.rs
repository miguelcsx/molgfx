use crate::fixture;
use crate::{CoreError, Scene, TrajectoryFrame, TrajectorySegment};
use std::sync::Arc;

fn segment(atom_count: usize, start_x: f32, end_x: f32) -> TrajectorySegment {
    let positions = |offset: f32| {
        Arc::from(
            (0..u16::try_from(atom_count).map_or(u16::MAX, |value| value))
                .map(|index| [offset + f32::from(index), 0.0, 0.0])
                .collect::<Vec<_>>(),
        )
    };
    let start = TrajectoryFrame::new(10, 1.0, positions(start_x), "test:start")
        .unwrap_or_else(|error| panic!("{error}"));
    let end = TrajectoryFrame::new(11, 2.0, positions(end_x), "test:end")
        .unwrap_or_else(|error| panic!("{error}"));
    TrajectorySegment::new(start, end, 1.25).unwrap_or_else(|error| panic!("{error}"))
}

fn scene() -> (Scene, crate::StructureHandle) {
    let structure = fixture::structure();
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    (scene, handle)
}

#[test]
fn a_scene_retains_a_topology_matched_two_frame_interval() {
    let (mut scene, handle) = scene();
    scene
        .set_trajectory_segment(handle, segment(8, -4.0, 12.0))
        .unwrap_or_else(|error| panic!("{error}"));
    let placed = scene.structure(handle).unwrap_or_else(|| panic!("placed"));
    assert_eq!(
        placed.trajectory().map(TrajectorySegment::atom_count),
        Some(8)
    );
    assert!(placed.world_aabb().min.x < -4.0);
    assert!(placed.world_aabb().max.x > 19.0);
}

#[test]
fn time_updates_reuse_the_pair_and_clear_restores_source_coordinates() {
    let (mut scene, handle) = scene();
    scene
        .set_trajectory_segment(handle, segment(8, -4.0, 12.0))
        .unwrap_or_else(|error| panic!("{error}"));
    let pair_bound = scene.world_aabb();
    let revision = scene
        .structure(handle)
        .map_or(0, crate::PlacedStructure::trajectory_revision);
    scene
        .set_trajectory_time(handle, 1.75)
        .unwrap_or_else(|error| panic!("{error}"));
    let placed = scene.structure(handle).unwrap_or_else(|| panic!("placed"));
    assert!(placed.trajectory_revision() > revision);
    assert_eq!(placed.world_aabb(), pair_bound);
    assert!(matches!(scene.clear_trajectory(handle), Ok(true)));
    assert!(
        scene
            .structure(handle)
            .is_some_and(|placed| placed.trajectory().is_none())
    );
    assert!(scene.world_aabb().max.x < pair_bound.max.x);
}

#[test]
fn a_mismatched_topology_is_rejected_without_mutating_the_scene() {
    let (mut scene, handle) = scene();
    let error = scene.set_trajectory_segment(handle, segment(7, 0.0, 1.0));
    assert!(matches!(error, Err(CoreError::InvalidTrajectory { .. })));
    assert!(
        scene
            .structure(handle)
            .is_some_and(|placed| placed.trajectory().is_none())
    );
}
