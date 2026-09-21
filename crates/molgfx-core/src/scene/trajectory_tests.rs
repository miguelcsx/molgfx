use crate::fixture;
use crate::{
    AtomPropertyMeaning, CoreError, GuideCap, GuideStyle, ScalarFieldSemantics, Scene,
    TrajectoryFrame, TrajectorySegment,
};
use std::sync::Arc;

fn segment(atom_count: usize, start_x: f32, end_x: f32) -> TrajectorySegment {
    let positions = |offset: f32| {
        Arc::from(
            (0..u16::try_from(atom_count).unwrap_or(u16::MAX))
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
fn trajectory_vectors_anchor_at_the_interpolated_position_and_scale_the_displacement() {
    let (mut scene, handle) = scene();
    scene
        .set_trajectory_segment(handle, segment(8, -4.0, 12.0))
        .unwrap_or_else(|error| panic!("{error}"));
    let handles = scene
        .add_trajectory_vectors(handle, &[0, 3], 0.5, 0.0, GuideStyle::default())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(handles.len(), 2);
    // Displacement is +16 in x for every atom; sample time 1.25 in [1.0, 2.0]
    // gives alpha 0.25, so the tail sits at from.x + 16*0.25 and the head a
    // further 16*0.5 along x.
    let first = scene.guide(handles[0]).unwrap_or_else(|| panic!("guide"));
    assert!((first.start().x - 0.0).abs() < 1e-4);
    assert!((first.end().x - 8.0).abs() < 1e-4);
    assert_eq!(first.style().cap, GuideCap::Arrow);
    let second = scene.guide(handles[1]).unwrap_or_else(|| panic!("guide"));
    assert!((second.start().x - 3.0).abs() < 1e-4);
    assert!((second.end().x - 11.0).abs() < 1e-4);
}

#[test]
fn trajectory_vectors_skip_atoms_at_or_below_the_threshold() {
    let (mut scene, handle) = scene();
    scene
        .set_trajectory_segment(handle, segment(8, -4.0, 12.0))
        .unwrap_or_else(|error| panic!("{error}"));
    // Every atom moves exactly 16 units; a larger floor drops them all.
    let handles = scene
        .add_trajectory_vectors(handle, &[0, 1, 2], 1.0, 16.5, GuideStyle::default())
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(handles.is_empty());
}

#[test]
fn displacement_property_holds_per_atom_motion_magnitude() {
    let (mut scene, handle) = scene();
    assert!(matches!(
        scene.trajectory_displacement_property(
            handle,
            "rmsf",
            ScalarFieldSemantics::UncalibratedRank
        ),
        Err(CoreError::InvalidTrajectory { .. })
    ));
    scene
        .set_trajectory_segment(handle, segment(8, -4.0, 12.0))
        .unwrap_or_else(|error| panic!("{error}"));
    let property = scene
        .trajectory_displacement_property(handle, "rmsf", ScalarFieldSemantics::UncalibratedRank)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(property.meaning(), AtomPropertyMeaning::Flexibility);
    // Every atom moves +16 in x between the two frames.
    assert_eq!(property.values().len(), 8);
    assert!(
        property
            .values()
            .iter()
            .all(|value| (value - 16.0).abs() < 1e-4)
    );
    // The derived column is length-valid against the owner and can be added.
    assert!(scene.add_atom_property(property).is_ok());
}

#[test]
fn trajectory_vectors_reject_bad_inputs() {
    let (mut scene, handle) = scene();
    assert!(matches!(
        scene.add_trajectory_vectors(handle, &[0], 1.0, 0.0, GuideStyle::default()),
        Err(CoreError::InvalidTrajectory { .. })
    ));
    scene
        .set_trajectory_segment(handle, segment(8, -4.0, 12.0))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(matches!(
        scene.add_trajectory_vectors(handle, &[0], 0.0, 0.0, GuideStyle::default()),
        Err(CoreError::InvalidAnnotation { .. })
    ));
    assert!(matches!(
        scene.add_trajectory_vectors(handle, &[99], 1.0, 0.0, GuideStyle::default()),
        Err(CoreError::InvalidTrajectory { .. })
    ));
}

#[test]
fn bond_break_length_is_stored_and_validated() {
    let (mut scene, handle) = scene();
    assert_eq!(
        scene
            .structure(handle)
            .map(crate::PlacedStructure::bond_break_length),
        Some(0.0)
    );
    scene
        .set_bond_break_length(handle, 3.0)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        scene
            .structure(handle)
            .map(crate::PlacedStructure::bond_break_length),
        Some(3.0)
    );
    assert!(matches!(
        scene.set_bond_break_length(handle, -1.0),
        Err(CoreError::InvalidTrajectory { .. })
    ));
    assert!(matches!(
        scene.set_bond_break_length(handle, f32::NAN),
        Err(CoreError::InvalidTrajectory { .. })
    ));
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
