use crate::{
    Color, DataSource, PatchOperation, PickKind, PickResult, ResolvedPick, Scene, ScenePatch,
    SegmentStyle, SegmentationBinding, SegmentationSpec,
};
use molgfx_math::Mat4;
use std::sync::Arc;

fn add(scene: &mut Scene, source: &str) -> crate::SegmentationId {
    let source = DataSource::new(source);
    let affine = Mat4::IDENTITY.to_cols_array();
    let id = scene
        .add_segmentation(SegmentationSpec {
            presentation: crate::SegmentationPresentation::Surface,
            source: source.clone(),
            dimensions: [2; 3],
            voxel_to_world: affine,
            styles: vec![SegmentStyle {
                label: 42,
                color: Color::rgb(220, 40, 60),
                opacity: 0.8,
                visible: true,
            }],
        })
        .unwrap();
    scene
        .bind_segmentation(
            SegmentationBinding::new(source, [2; 3], affine, Arc::from([42; 8])).unwrap(),
        )
        .unwrap();
    id
}

#[test]
fn two_segmentations_with_the_same_label_resolve_to_their_own_ids() {
    let mut scene = Scene::empty();
    let first = add(&mut scene, "first");
    let second = add(&mut scene, "second");
    for (id, handle) in &scene.overlay.segmentations {
        let resolved = scene
            .resolve_pick(&PickResult {
                kind: PickKind::VolumeSegment,
                dataset: None,
                chunk: None,
                row: None,
                volume_label: Some(42),
                segmentation: Some(*handle),
                source_id: Some(scene.resolved.cache_identity()),
            })
            .unwrap();
        let ResolvedPick::VolumeSegment(segment) = resolved else {
            panic!("categorical pick must resolve");
        };
        assert_eq!(segment.segmentation, *id);
        assert!([first, second].contains(&segment.segmentation));
        let json = serde_json::to_value(segment).unwrap();
        assert!(json.get("segmentation").is_some());
        assert!(json.get("volume").is_none());
    }
}

#[test]
fn segment_style_inverse_preserves_label_handles_and_content_revision() {
    let mut scene = Scene::empty();
    let id = add(&mut scene, "labels");
    let handle = scene.overlay.segmentations[0].1;
    let revision = scene.resolved.segmentation_content_revision(handle);
    let base = scene.spec.clone();
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::SetSegmentStyles { id, styles: vec![] }],
    };
    let inverse = patch.inverse(&base).unwrap();
    scene.apply(&patch).unwrap();
    assert_eq!(scene.overlay.segmentations[0].1, handle);
    assert_eq!(
        scene.resolved.segmentation_content_revision(handle),
        revision
    );
    assert!(
        scene
            .resolved
            .representations()
            .all(|(_, representation)| !representation.visible)
    );
    scene.apply(&inverse).unwrap();
    assert_eq!(
        scene.spec.segmentations[&id].styles,
        base.segmentations[&id].styles
    );
}

#[test]
fn reused_segmentation_slots_reject_the_removed_generation() {
    let mut scene = Scene::empty();
    let first = add(&mut scene, "first");
    scene.remove_segmentation(first).unwrap();
    let second = add(&mut scene, "second");
    assert_eq!(first.index, second.index);
    assert_ne!(first.generation, second.generation);
    assert!(scene.set_segment_styles(first, vec![]).is_err());
    let decoded = crate::SceneSpec::from_json(&scene.spec.to_json().unwrap()).unwrap();
    assert!(decoded.segmentations.contains_key(&second));
}

#[test]
fn a_captured_categorical_pick_is_stale_after_a_grid_is_removed_and_replaced() {
    let mut scene = Scene::empty();
    let first = add(&mut scene, "first");
    let captured = PickResult {
        kind: PickKind::VolumeSegment,
        dataset: None,
        chunk: None,
        row: None,
        volume_label: Some(42),
        segmentation: Some(scene.overlay.segmentations[0].1),
        source_id: Some(scene.resolved.cache_identity()),
    };
    scene.remove_segmentation(first).unwrap();
    let _ = add(&mut scene, "second");
    assert!(matches!(
        scene.resolve_pick(&captured),
        Err(crate::Error::StalePick { .. })
    ));
}

#[test]
fn removed_segmentation_inverse_restores_the_descriptor_and_shared_binding() {
    let mut scene = Scene::empty();
    let id = add(&mut scene, "labels");
    let base = scene.spec.clone();
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::RemoveSegmentation { id }],
    };
    let inverse = patch.inverse(&base).unwrap();
    scene.apply(&patch).unwrap();
    assert_eq!(scene.overlay_handles().segmentations, 0);
    scene.apply(&inverse).unwrap();
    assert_eq!(scene.spec.segmentations[&id], base.segmentations[&id]);
    assert_eq!(scene.overlay_handles().segmentations, 1);
}

#[test]
fn a_failed_categorical_binding_leaves_its_source_available_for_a_correct_retry() {
    let mut scene = Scene::empty();
    let spec = SegmentationSpec {
        presentation: crate::SegmentationPresentation::Surface,
        source: DataSource::new("labels"),
        dimensions: [2; 3],
        voxel_to_world: Mat4::IDENTITY.to_cols_array(),
        styles: vec![],
    };
    let _ = scene.add_segmentation(spec.clone()).unwrap();
    let before = scene.spec.clone();
    let mut wrong = spec.voxel_to_world;
    wrong[12] = 1.0;
    assert!(
        scene
            .bind_segmentation(
                SegmentationBinding::new(
                    spec.source.clone(),
                    spec.dimensions,
                    wrong,
                    Arc::from([42; 8])
                )
                .unwrap()
            )
            .is_err()
    );
    assert_eq!(scene.spec, before);
    scene
        .bind_segmentation(
            SegmentationBinding::new(
                spec.source,
                spec.dimensions,
                spec.voxel_to_world,
                Arc::from([42; 8]),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(scene.overlay_handles().segmentations, 1);
}
