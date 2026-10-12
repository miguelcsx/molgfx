use super::Scene;
use crate::{Color, DataSource, VolumeBinding, VolumeId, density};
use std::sync::Arc;

#[test]
fn malformed_bulk_grids_leave_the_scene_and_the_next_volume_identity_unchanged() {
    let mut scene = Scene::from_structure(&super::tests::structure()).expect("structure");
    let initial = scene.to_spec();
    let source = DataSource::new("atomic-grid");
    let volume =
        density::volume(source.clone(), [2; 3]).isosurface(0.5, Color::rgb(0, 80, 255), 1.0);
    assert!(
        scene
            .add_volume_with_binding(
                volume.clone(),
                VolumeBinding::new(source.clone(), [2; 3], Arc::from([0.0; 7]))
            )
            .is_err()
    );
    assert_eq!(scene.to_spec(), initial);
    assert!(scene.unresolved_overlays().is_empty());
    let values = Arc::from([0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    let id = scene
        .add_volume_with_binding(volume, VolumeBinding::new(source, [2; 3], values))
        .expect("complete grid");
    assert_eq!(id, VolumeId::new(1));
    assert_eq!(scene.revision(), initial.revision + 1);
    assert!(scene.unresolved_overlays().is_empty());
}

#[test]
fn invalid_presentations_do_not_consume_the_bulk_source_binding() {
    let mut scene = Scene::from_structure(&super::tests::structure()).expect("structure");
    let initial = scene.to_spec();
    let source = DataSource::new("same-grid-after-rejection");
    let values: Arc<[f32]> = Arc::from([0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    let invalid =
        density::volume(source.clone(), [2; 3]).slice([0.0; 3], [0.0; 3], "viridis", [0.0, 1.0]);
    assert!(
        scene
            .add_volume_with_binding(
                invalid,
                VolumeBinding::new(source.clone(), [2; 3], Arc::clone(&values))
            )
            .is_err()
    );
    assert_eq!(scene.to_spec(), initial);
    let valid = density::volume(source.clone(), [2; 3]).slice(
        [0.0; 3],
        [0.0, 0.0, 1.0],
        "viridis",
        [0.0, 1.0],
    );
    assert_eq!(
        scene
            .add_volume_with_binding(valid, VolumeBinding::new(source, [2; 3], values))
            .expect("valid slice"),
        VolumeId::new(1)
    );
}

#[test]
fn a_bulk_insert_rejects_a_binding_for_another_source_before_mutating_the_scene() {
    let mut scene = Scene::from_structure(&super::tests::structure()).expect("structure");
    let initial = scene.to_spec();
    let volume = density::volume(DataSource::new("descriptor"), [2; 3]).isosurface(
        0.5,
        Color::rgb(0, 80, 255),
        1.0,
    );
    let binding = VolumeBinding::new(
        DataSource::new("different-source"),
        [2; 3],
        Arc::from([0.0; 8]),
    );
    assert!(scene.add_volume_with_binding(volume, binding).is_err());
    assert_eq!(scene.to_spec(), initial);
    assert!(scene.unresolved_overlays().is_empty());
}
