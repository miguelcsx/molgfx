use super::*;
use crate::{TrajectoryFrame, TrajectorySegment};
use std::sync::Arc;

fn scene_with_frames(shift: f32) -> (Scene, StructureHandle, TrajectoryFrame, TrajectoryFrame) {
    let structure = crate::fixture::structure();
    let scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let start_positions: Arc<[[f32; 3]]> = Arc::from(placed.atoms.coords().slice());
    let mut end_positions = start_positions.to_vec();
    for position in &mut end_positions {
        position[0] += shift;
    }
    let start = TrajectoryFrame::new(3, 2.0, start_positions, "from")
        .unwrap_or_else(|error| panic!("{error}"));
    let end = TrajectoryFrame::new(9, 6.0, Arc::from(end_positions), "to")
        .unwrap_or_else(|error| panic!("{error}"));
    (scene, owner, start, end)
}

fn interpolation(scene: &Scene, owner: StructureHandle) -> Option<f64> {
    scene
        .structure(owner)
        .and_then(|value| value.trajectory())
        .map(TrajectorySegment::interpolation)
}

#[test]
fn a_morph_shows_the_first_frame_at_the_start_and_the_second_at_the_end() {
    let (mut scene, owner, from, to) = scene_with_frames(1.0);
    let mut timeline = Timeline::new();
    timeline
        .morph(&mut scene, owner, from, to, 4.0)
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 0.0)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(interpolation(&scene, owner), Some(0.0));
    timeline
        .apply(&mut scene, 4.0)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(interpolation(&scene, owner), Some(1.0));
    timeline
        .apply(&mut scene, 40.0)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(interpolation(&scene, owner), Some(1.0));
}

#[test]
fn a_morph_blends_the_two_frames_halfway_at_the_midpoint() {
    let (mut scene, owner, from, to) = scene_with_frames(1.0);
    let mut timeline = Timeline::new();
    timeline
        .morph(&mut scene, owner, from, to, 4.0)
        .unwrap_or_else(|error| panic!("{error}"));
    timeline
        .apply(&mut scene, 2.0)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(segment) = scene.structure(owner).and_then(|value| value.trajectory()) else {
        panic!("morph installs a segment")
    };
    assert_eq!(segment.interpolation().to_bits(), 0.5f64.to_bits());
    assert_eq!(segment.start().provenance(), "from");
    assert_eq!(segment.end().provenance(), "to");
}

#[test]
fn a_second_trajectory_track_for_the_structure_is_refused_and_changes_nothing() {
    let (mut scene, owner, from, to) = scene_with_frames(1.0);
    let mut timeline = Timeline::new();
    timeline
        .morph(&mut scene, owner, from.clone(), to.clone(), 4.0)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(timeline.has_trajectory_track(owner));
    assert!(timeline.morph(&mut scene, owner, from, to, 4.0).is_err());
}

#[test]
fn a_morph_with_a_bad_duration_leaves_the_scene_and_timeline_untouched() {
    let (mut scene, owner, from, to) = scene_with_frames(1.0);
    let mut timeline = Timeline::new();
    assert!(timeline.morph(&mut scene, owner, from, to, 0.0).is_err());
    assert!(!timeline.has_trajectory_track(owner));
    assert!(
        scene
            .structure(owner)
            .is_some_and(|value| value.trajectory().is_none())
    );
}
