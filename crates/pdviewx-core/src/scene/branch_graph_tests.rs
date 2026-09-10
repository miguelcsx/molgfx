use super::*;
use crate::TrajectoryFrame;
use std::sync::Arc;

fn segment(scene: &Scene, owner: StructureHandle, offset: f32) -> TrajectorySegment {
    let coordinates: Arc<[[f32; 3]]> = scene.structure(owner).map_or_else(
        || panic!("owner"),
        |placed| Arc::from(placed.atoms.coords().slice()),
    );
    let end = coordinates
        .iter()
        .map(|position| [position[0] + offset, position[1], position[2]])
        .collect::<Vec<_>>();
    TrajectorySegment::new(
        TrajectoryFrame::new(0, 0.0, coordinates, "branch:start")
            .unwrap_or_else(|error| panic!("{error}")),
        TrajectoryFrame::new(1, 1.0, Arc::from(end.into_boxed_slice()), "branch:end")
            .unwrap_or_else(|error| panic!("{error}")),
        0.0,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn graph_switches_only_after_the_scene_accepts_a_decoded_segment() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let owner = scene
        .structures()
        .next()
        .map_or_else(|| panic!("owner"), |(handle, _)| handle);
    let branch =
        TrajectoryBranch::new(10, 20, "lower-ph", 0.35).unwrap_or_else(|error| panic!("{error}"));
    let mut graph = TrajectoryStateGraph::new(vec![20, 10], vec![branch], 10)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(graph.target("lower-ph"), Some(20));
    let decoded = segment(&scene, owner, 2.0);
    let selected = graph
        .transition(&mut scene, owner, "lower-ph", decoded)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(selected, 20);
    assert_eq!(graph.active(), 20);
    assert!(
        scene
            .structure(owner)
            .and_then(|placed| placed.trajectory())
            .is_some()
    );
}

#[test]
fn unknown_events_do_not_change_the_active_state() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let owner = scene
        .structures()
        .next()
        .map_or_else(|| panic!("owner"), |(handle, _)| handle);
    let mut graph =
        TrajectoryStateGraph::new(vec![1], Vec::new(), 1).unwrap_or_else(|error| panic!("{error}"));
    let decoded = segment(&scene, owner, 1.0);
    let result = graph.transition(&mut scene, owner, "absent", decoded);
    assert!(matches!(result, Err(CoreError::InvalidTimeline { .. })));
    assert_eq!(graph.active(), 1);
}
