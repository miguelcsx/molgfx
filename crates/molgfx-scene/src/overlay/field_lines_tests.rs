use crate::{DataSource, FieldLines, PatchOperation, Scene, ScenePatch, VolumeBinding, density};
use std::sync::Arc;

fn values() -> Arc<[f32]> {
    Arc::from(
        (0..125_u16)
            .map(|row| f32::from(row % 5))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn field_lines_bind_without_a_molecular_structure_and_remove_with_their_volume() {
    let mut scene = Scene::empty();
    let source = DataSource::new("linear-potential");
    let options = FieldLines {
        seed_stride: 2,
        step: 0.125,
        length: [0.25, 4.0],
        ..FieldLines::default()
    };
    let id = scene
        .add(density::volume(source.clone(), [5; 3]).field_lines(options))
        .unwrap();
    assert_eq!(scene.unresolved_overlays().len(), 1);
    scene
        .bind_volume(VolumeBinding::new(source, [5; 3], values()))
        .unwrap();
    assert!(scene.unresolved_overlays().is_empty());
    assert_eq!(scene.resolved().relation_batches().count(), 1);
    assert_eq!(scene.resolved().representations().count(), 0);
    scene
        .apply(&ScenePatch {
            base_revision: scene.revision(),
            operations: vec![PatchOperation::RemoveVolume { id }],
        })
        .unwrap();
    assert_eq!(scene.resolved().relation_batches().count(), 0);
}

#[test]
fn field_line_budget_failure_keeps_the_previous_resolved_scene() {
    let mut scene = Scene::empty();
    let source = DataSource::new("linear-potential");
    let options = FieldLines {
        seed_stride: 2,
        max_segments: 1,
        ..FieldLines::default()
    };
    scene
        .add(density::volume(source.clone(), [5; 3]).field_lines(options))
        .unwrap();
    let before = scene.to_spec();
    assert!(
        scene
            .bind_volume(VolumeBinding::new(source, [5; 3], values()))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
    assert_eq!(scene.resolved().relation_batches().count(), 0);
    assert_eq!(scene.unresolved_overlays().len(), 1);
}
