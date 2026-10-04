use super::Scene;
use super::tests::structure;
use crate::interop::SceneSnapshot;
use crate::{
    Error, PatchOperation, PlaneId, PlaneSpec, ScenePatch, SceneSpec, StructureId, rep, sel,
};

fn scene() -> Scene {
    Scene::from_structure(&structure()).expect("fixture resolves")
}

fn authored(spec: &SceneSpec) -> SceneSpec {
    let mut spec = spec.clone();
    spec.revision = 0;
    spec
}

#[test]
fn restoring_a_snapshot_recovers_camera_styles_and_overlays_and_undo_returns() {
    let mut scene = scene();
    let id = scene
        .add(rep::spacefill(sel::all()))
        .expect("representation");
    let camera = scene.framing_camera(1.0);
    scene.set_camera(Some(camera)).expect("camera");
    let plane = PlaneSpec::new(
        StructureId::new(1),
        [11.0, 6.0, -6.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [4.0, 4.0],
    );
    scene
        .apply(&ScenePatch {
            base_revision: scene.revision(),
            operations: vec![PatchOperation::AddPlane {
                id: PlaneId::new(1),
                spec: plane,
            }],
        })
        .expect("plane");
    let snapshot = scene.snapshot();
    let wire = snapshot.to_json().expect("snapshot transport");
    let snapshot = SceneSnapshot::from_json(&wire).expect("snapshot round trip");
    scene.set_visible(id, false).expect("visibility");
    scene.set_opacity(id, 0.2).expect("opacity");
    scene.set_camera(None).expect("camera reset");
    scene
        .apply(&ScenePatch {
            base_revision: scene.revision(),
            operations: vec![PatchOperation::RemovePlane {
                id: PlaneId::new(1),
            }],
        })
        .expect("remove plane");
    let before = scene.to_spec();
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::RestoreSnapshot(Box::new(snapshot.clone()))],
    };
    let mut inverse = patch.inverse(&before).expect("inverse");
    let patch = ScenePatch::from_json(&patch.to_json().expect("patch transport"))
        .expect("patch round trip");
    scene.apply(&patch).expect("restore");
    assert_eq!(authored(scene.spec()), authored(&snapshot.scene));
    assert_eq!(scene.revision(), before.revision + 1);
    assert_eq!(scene.overlay_handles().plane_guides, 4);
    inverse.base_revision = scene.revision();
    scene.apply(&inverse).expect("undo restore");
    assert_eq!(authored(scene.spec()), authored(&before));
    assert_eq!(scene.revision(), before.revision + 2);
    assert_eq!(scene.overlay_handles().plane_guides, 0);
}

#[test]
fn missing_mismatched_or_tampered_snapshot_sources_leave_live_state_unchanged() {
    let mut scene = scene();
    let before = scene.to_spec();
    let mut missing = before.clone();
    let descriptor = missing.structures[&StructureId::new(1)].clone();
    let _ = missing.structures.insert(StructureId::new(99), descriptor);
    assert!(
        matches!(scene.restore_snapshot(&SceneSnapshot::capture(&missing)), Err(Error::MissingSource { id }) if id == StructureId::new(99))
    );
    assert_eq!(scene.spec(), &before);
    let mut mismatch = before.clone();
    mismatch
        .structures
        .get_mut(&StructureId::new(1))
        .expect("source")
        .content_hash = "wrong".into();
    assert!(
        scene
            .restore_snapshot(&SceneSnapshot::capture(&mismatch))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
    let mut tampered = scene.snapshot();
    tampered.scene.revision += 1;
    assert!(scene.restore_snapshot(&tampered).is_err());
    assert_eq!(scene.spec(), &before);
}

#[test]
fn undo_can_reattach_sources_removed_by_a_snapshot_without_reusing_their_ids() {
    let mut scene = scene();
    let snapshot = scene.snapshot();
    let later = scene.add_structure(&structure()).expect("second source");
    let _ = scene
        .add(rep::spacefill(sel::all()).structure(later))
        .expect("second representation");
    let before = scene.to_spec();
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::RestoreSnapshot(Box::new(snapshot))],
    };
    let inverse = patch.inverse(&before).expect("inverse");
    scene.apply(&patch).expect("restore");
    assert_eq!(scene.sources().len(), 1);
    assert_eq!(scene.resolved().structures().count(), 1);
    scene.apply(&inverse).expect("reattach inverse sources");
    assert_eq!(authored(scene.spec()), authored(&before));
    assert_eq!(scene.sources().len(), 2);
    let next = scene.add_structure(&structure()).expect("new identity");
    assert!(next.get() > later.get());
}

#[test]
fn a_later_restore_cannot_hide_an_earlier_snapshot_with_a_mismatched_volume() {
    use crate::{Color, DataSource, VolumeBinding, density};
    use std::sync::Arc;

    let mut scene = Scene::empty();
    let source = DataSource::new("snapshot-grid");
    let id = scene
        .add(density::volume(source.clone(), [2; 3]).isosurface(0.5, Color::rgb(255, 0, 0), 1.0))
        .expect("volume descriptor");
    scene
        .bind_volume(VolumeBinding::new(source, [2; 3], Arc::from([0.0; 8])))
        .expect("volume binding");
    let before = scene.to_spec();
    let valid = scene.snapshot();
    let mut mismatched = before.clone();
    mismatched.volumes.get_mut(&id).expect("volume").dimensions = [3; 3];
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![
            PatchOperation::RestoreSnapshot(Box::new(SceneSnapshot::capture(&mismatched))),
            PatchOperation::RestoreSnapshot(Box::new(valid)),
        ],
    };
    assert!(scene.apply(&patch).is_err());
    assert_eq!(scene.spec(), &before);
    assert_eq!(scene.overlay_handles().volumes, 1);
    assert_eq!(scene.resolved().representations().count(), 1);
}

#[test]
fn a_later_restore_cannot_hide_an_earlier_snapshot_with_a_mismatched_trajectory() {
    use crate::{DataSource, TrajectoryBinding, TrajectoryFrame, trajectory};
    use std::sync::Arc;

    let mut scene = scene();
    let source = DataSource::new("snapshot-frames");
    let id = scene
        .add(trajectory::trajectory(
            StructureId::new(1),
            source.clone(),
            3,
        ))
        .expect("trajectory descriptor");
    scene
        .bind_trajectory(TrajectoryBinding::new(
            source,
            TrajectoryFrame::new(0, 0.0, Arc::from([[11.0, 6.0, -6.0]])),
            TrajectoryFrame::new(2, 2.0, Arc::from([[12.0, 6.0, -6.0]])),
        ))
        .expect("trajectory binding");
    let before = scene.to_spec();
    let valid = scene.snapshot();
    let mut mismatched = before.clone();
    mismatched
        .trajectories
        .get_mut(&id)
        .expect("trajectory")
        .frame_count = 2;
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![
            PatchOperation::RestoreSnapshot(Box::new(SceneSnapshot::capture(&mismatched))),
            PatchOperation::RestoreSnapshot(Box::new(valid)),
        ],
    };
    assert!(scene.apply(&patch).is_err());
    assert_eq!(scene.spec(), &before);
    assert_eq!(scene.overlay_handles().trajectories, 1);
}

#[test]
fn a_later_restore_cannot_hide_an_earlier_snapshot_with_an_unbound_property() {
    use crate::{DataSource, PropertySpec};

    let mut scene = scene();
    let before = scene.to_spec();
    let valid = scene.snapshot();
    let mut missing = before.clone();
    let _ = missing.properties.insert(
        "unbound".into(),
        PropertySpec {
            structure: StructureId::new(1),
            source: DataSource::new("missing-column"),
            domain: [0.0, 1.0],
            units: None,
        },
    );
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![
            PatchOperation::RestoreSnapshot(Box::new(SceneSnapshot::capture(&missing))),
            PatchOperation::RestoreSnapshot(Box::new(valid)),
        ],
    };
    assert!(scene.apply(&patch).is_err());
    assert_eq!(scene.spec(), &before);
}

#[test]
fn a_later_restore_cannot_hide_an_earlier_snapshot_with_an_unknown_derived_column() {
    use crate::{DataSource, PropertySpec};

    let mut scene = scene();
    let before = scene.to_spec();
    let valid = scene.snapshot();
    let mut missing = before.clone();
    let _ = missing.properties.insert(
        "unknown-derived".into(),
        PropertySpec {
            structure: StructureId::new(1),
            source: DataSource::new("missing-column")
                .format("derived")
                .uri("unknown-column"),
            domain: [0.0, 1.0],
            units: None,
        },
    );
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![
            PatchOperation::RestoreSnapshot(Box::new(SceneSnapshot::capture(&missing))),
            PatchOperation::RestoreSnapshot(Box::new(valid)),
        ],
    };
    assert!(scene.apply(&patch).is_err());
    assert_eq!(scene.spec(), &before);
}
