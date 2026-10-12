use crate::{Scene, StructureId, TrajectoryFrame};
use std::sync::Arc;

#[test]
fn retained_frames_share_one_validation_cache_without_changing_value_equality() {
    let frame = TrajectoryFrame::new(3, 3.0, Arc::from([[1.0; 3]]));
    let cloned = frame.clone();
    let equal = TrajectoryFrame::new(3, 3.0, Arc::from([[1.0; 3]]));
    assert_eq!(frame, equal);
    assert!(frame.native.get().is_none());
    assert!(Arc::ptr_eq(&frame.native, &cloned.native));
    let native = cloned.native().expect("validated column");
    assert!(frame.native.get().is_some());
    assert!(equal.native.get().is_none());
    assert_eq!(frame, equal);
    assert_eq!(native.positions().as_ptr(), frame.positions().as_ptr());
    assert_eq!(
        frame.native().expect("cached frame").positions().as_ptr(),
        native.positions().as_ptr()
    );
    let invalid = TrajectoryFrame::new(4, 4.0, Arc::from([[f32::NAN; 3]]));
    assert!(invalid.native().is_err());
    assert!(invalid.native.get().is_none());
}

fn first_placed(scene: &Scene) -> &molgfx_core::PlacedStructure {
    scene.resolved().structures().next().expect("placed").1
}

#[test]
fn resupplying_a_retained_pair_updates_only_time_and_refuses_an_invalid_sample_atomically() {
    let (source, _) = molframe::read_bytes(
        b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n"
            .to_vec(),
        Some("pair.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source");
    let mut scene = Scene::from_structure(&source).expect("scene");
    let id = StructureId::new(1);
    let a = TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]]));
    let b = TrajectoryFrame::new(1, 1.0, Arc::from([[2.0, 0.0, 0.0]]));
    scene
        .set_trajectory_pair(id, a.clone(), b.clone(), 0.0)
        .expect("interval");
    let pair_revision = first_placed(&scene).trajectory_pair_revision();
    let pose_revision = first_placed(&scene).trajectory_revision();
    scene
        .set_trajectory_pair(id, a.clone(), b.clone(), 0.5)
        .expect("same retained interval");
    assert_eq!(
        first_placed(&scene).trajectory_pair_revision(),
        pair_revision
    );
    assert_eq!(
        first_placed(&scene).trajectory_revision(),
        pose_revision + 1
    );
    let pose = scene.molecular_pose(id).expect("pose");
    assert!(
        scene
            .set_trajectory_pair(id, a.clone(), b.clone(), 2.0)
            .is_err()
    );
    assert_eq!(scene.molecular_pose(id).expect("unchanged"), pose);
    scene.set_trajectory_pair(id, a, b, 0.5).expect("same time");
    assert_eq!(
        first_placed(&scene).trajectory_revision(),
        pose_revision + 1
    );
    let a = TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]]));
    let b = TrajectoryFrame::new(1, 1.0, Arc::from([[2.0, 0.0, 0.0]]));
    scene
        .set_trajectory_pair(id, a, b, 0.5)
        .expect("different storage requires validation and refit");
    assert_eq!(
        first_placed(&scene).trajectory_pair_revision(),
        pair_revision + 1
    );
}
