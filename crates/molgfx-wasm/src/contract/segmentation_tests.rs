use super::WebScene;
use molgfx::{Color, SegmentStyle, SegmentationId, SegmentationSpec};

#[wasm_bindgen_test::wasm_bindgen_test]
fn browser_segmentation_ingestion_preserves_labels_and_style_only_updates() {
    let mut scene = WebScene::empty();
    let spec = SegmentationSpec {
        presentation: molgfx::SegmentationPresentation::Surface,
        source: molgfx::schema::DataSource::new("labels"),
        dimensions: [2, 2, 2],
        voxel_to_world: [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0,
        ],
        styles: vec![SegmentStyle {
            label: u32::MAX,
            color: Color::rgb(255, 0, 0),
            opacity: 1.0,
            visible: true,
        }],
    };
    let specification = serde_json::to_string(&spec).expect("specification");
    let identity = scene
        .add_segmentation(&specification)
        .expect("authored segmentation");
    let id: SegmentationId = serde_json::from_str(&identity).expect("generational identity");
    scene
        .bind_segmentation(id.index, id.generation, vec![u32::MAX; 8])
        .expect("unsigned label grid");
    scene
        .resolve()
        .expect("labels persist across re-resolution");
    assert_eq!(scene.spec.segmentations[&id], spec);
    scene
        .set_segment_styles(id.index, id.generation, "[]")
        .expect("empty styles");
    assert!(scene.spec.segmentations[&id].styles.is_empty());
    assert!(
        scene
            .resolved
            .as_ref()
            .expect("resolved")
            .unresolved_overlays()
            .is_empty()
    );
    scene
        .remove_segmentation(id.index, id.generation)
        .expect("remove");
    scene.resolve().expect("removed binding stays absent");
    assert!(scene.spec.segmentations.is_empty());
}
