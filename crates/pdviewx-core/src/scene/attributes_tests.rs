use crate::{
    AttributeColumn, AttributeValues, PointBatch, PointGlyph, PointStyle, RowDomain, Scene,
    SourceNamespace, SourceRows,
};
use std::sync::Arc;

#[test]
fn attribute_target_mismatch_is_atomic_and_dirty_ranges_are_exact() {
    let mut scene = Scene::new();
    let points = PointBatch::new(
        Arc::from([[0.0; 3], [1.0; 3], [2.0; 3]]),
        SourceRows::ordered(SourceNamespace(1), 3),
        PointGlyph::Disc,
        PointStyle::default(),
    )
    .unwrap();
    let point_handle = scene.add_point_batch(points);
    let domain = RowDomain::Points(point_handle);
    let bad = AttributeColumn::new(
        domain,
        "bad",
        AttributeValues::Scalar(Arc::from([1.0, 2.0])),
    )
    .unwrap();
    assert!(scene.add_attribute(bad).is_err());
    assert_eq!(scene.attributes().count(), 0);

    let initial = AttributeColumn::new(
        domain,
        "score",
        AttributeValues::Scalar(Arc::from([1.0, 2.0, 3.0])),
    )
    .unwrap();
    let handle = scene.add_attribute(initial).unwrap();
    let replacement = AttributeColumn::new(
        domain,
        "score",
        AttributeValues::Scalar(Arc::from([1.0, 5.0, 3.0])),
    )
    .unwrap();
    scene
        .replace_attribute_range(handle, replacement, 1..2)
        .unwrap();
    assert_eq!(scene.attribute_change(handle).unwrap().1, 1..2);
}
