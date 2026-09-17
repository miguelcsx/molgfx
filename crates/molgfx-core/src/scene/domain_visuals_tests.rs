use crate::{
    AttributeColumn, AttributeValues, PointBatch, PointGlyph, PointStyle, Relation, RelationBatch,
    RelationStyle, RowDomain, Scene, SourceNamespace, SourceRows, SpatialAnchor, VisualDescriptor,
    VisualProgramBuilder, VisualStyle,
};
use molgfx_math::Vec3;
use std::sync::Arc;

fn points(scene: &mut Scene, namespace: u64) -> RowDomain {
    let batch = PointBatch::new(
        Arc::from([[0.0; 3], [1.0, 0.0, 0.0]]),
        SourceRows::ordered(SourceNamespace(namespace), 2),
        PointGlyph::Sphere,
        PointStyle::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    RowDomain::Points(scene.add_point_batch(batch))
}

#[test]
fn descriptor_attributes_must_target_the_exact_domain() {
    let mut scene = Scene::new();
    let first = points(&mut scene, 1);
    let second = points(&mut scene, 2);
    let attribute = AttributeColumn::new(
        first,
        "priority",
        AttributeValues::Scalar(Arc::from([0.2, 0.8])),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .add_attribute(attribute)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut builder = VisualProgramBuilder::new();
    let radius = builder
        .scalar_attribute(handle)
        .unwrap_or_else(|error| panic!("{error}"));
    builder
        .set_radius_scale(radius)
        .unwrap_or_else(|error| panic!("{error}"));
    let descriptor = VisualDescriptor::new(VisualStyle::new(
        builder.finish().unwrap_or_else(|error| panic!("{error}")),
    ));

    assert!(scene.set_domain_visual(second, descriptor.clone()).is_err());
    assert!(scene.domain_visual(second).is_none());
    scene
        .set_domain_visual(first, descriptor)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.domain_visuals().count(), 1);
}

#[test]
fn relation_descriptors_reject_radius_but_accept_width() {
    let relation = Relation {
        start: SpatialAnchor::world(Vec3::ZERO).unwrap_or_else(|error| panic!("{error}")),
        end: SpatialAnchor::world(Vec3::X).unwrap_or_else(|error| panic!("{error}")),
    };
    let batch = RelationBatch::new(
        Arc::from([relation]),
        SourceRows::ordered(SourceNamespace(3), 1),
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut scene = Scene::new();
    let domain = RowDomain::Relations(
        scene
            .add_relation_batch(batch)
            .unwrap_or_else(|error| panic!("{error}")),
    );

    let mut radius_builder = VisualProgramBuilder::new();
    let radius = radius_builder
        .scalar(2.0)
        .unwrap_or_else(|error| panic!("{error}"));
    radius_builder
        .set_radius_scale(radius)
        .unwrap_or_else(|error| panic!("{error}"));
    let radius = VisualDescriptor::new(VisualStyle::new(
        radius_builder
            .finish()
            .unwrap_or_else(|error| panic!("{error}")),
    ));
    assert!(scene.set_domain_visual(domain, radius).is_err());

    let mut width_builder = VisualProgramBuilder::new();
    let width = width_builder
        .scalar(2.0)
        .unwrap_or_else(|error| panic!("{error}"));
    width_builder
        .set_width_scale(width)
        .unwrap_or_else(|error| panic!("{error}"));
    let width = VisualDescriptor::new(VisualStyle::new(
        width_builder
            .finish()
            .unwrap_or_else(|error| panic!("{error}")),
    ));
    scene
        .set_domain_visual(domain, width)
        .unwrap_or_else(|error| panic!("{error}"));
}
