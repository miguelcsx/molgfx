use crate::{
    AnalyticSphere, AnalyticTemplate, AttributeColumn, AttributeValues,
    GenericSceneDescriptionSources, InstanceBatch, PointBatch, PointGlyph, PointStyle, Relation,
    RelationBatch, RelationStyle, RigidInstance, RowDomain, RowEntityRef, Scene,
    SceneDescriptionSources, SourceNamespace, SourceRows, SpatialAnchor, TemplatePartRef,
    VisualDescriptor, VisualProgramBuilder, VisualStyle,
};
use pdviewx_math::{Quat, Vec3};
use std::sync::Arc;

#[test]
fn schema_eight_round_trips_generic_identity_payloads_and_visuals() {
    let mut scene = Scene::new();
    let point_batch = point_batch(10, &[101, 103]);
    let point_handle = scene.add_point_batch(point_batch);
    let instance_batch = instance_batch();
    let instance_handle = scene.add_instance_batch(instance_batch);

    let attribute = AttributeColumn::new(
        RowDomain::Points(point_handle),
        "priority",
        AttributeValues::Scalar(Arc::from([0.25, 0.75])),
    )
    .unwrap_or_else(|error| panic!("attribute validates: {error}"));
    let attribute_handle = scene
        .add_attribute(attribute)
        .unwrap_or_else(|error| panic!("attribute attaches: {error}"));
    let relation_batch = relations(point_handle, instance_handle);
    let relation_handle = scene
        .add_relation_batch(relation_batch)
        .unwrap_or_else(|error| panic!("relations attach: {error}"));

    let mut builder = VisualProgramBuilder::new();
    let priority = builder
        .scalar_attribute(attribute_handle)
        .unwrap_or_else(|error| panic!("attribute input builds: {error}"));
    builder
        .set_radius_scale(priority)
        .unwrap_or_else(|error| panic!("visual output builds: {error}"));
    let descriptor = VisualDescriptor::new(VisualStyle::new(
        builder
            .finish()
            .unwrap_or_else(|error| panic!("visual program builds: {error}")),
    ))
    .with_order(7);
    scene
        .set_domain_visual(RowDomain::Points(point_handle), descriptor)
        .unwrap_or_else(|error| panic!("visual attaches: {error}"));

    let description = scene.describe();
    let point_sources: Vec<_> = scene
        .point_batches()
        .map(|(_, value)| value.clone())
        .collect();
    let instance_sources: Vec<_> = scene
        .instance_batches()
        .map(|(_, value)| value.clone())
        .collect();
    let attribute_sources: Vec<_> = scene.attributes().map(|(_, value)| value.clone()).collect();
    let relation_sources: Vec<_> = scene
        .relation_batches()
        .map(|(_, value)| value.clone())
        .collect();
    let manifest = scene.manifest(Vec::new());
    assert_eq!(manifest.payloads.len(), 4);

    let rebuilt = Scene::from_description_with_generic(
        &description,
        empty_sources(),
        GenericSceneDescriptionSources {
            point_batches: &point_sources,
            instance_batches: &instance_sources,
            attributes: &attribute_sources,
            relation_batches: &relation_sources,
        },
    )
    .unwrap_or_else(|error| panic!("generic scene rehydrates: {error}"));

    assert_eq!(rebuilt.describe(), description);
    assert_eq!(
        rebuilt
            .point_batch(point_handle)
            .and_then(|value| value.source_rows().key(1)),
        Some(103)
    );
    assert_eq!(
        rebuilt
            .instance_batch(instance_handle)
            .and_then(|value| value.template().source_rows().key(1)),
        Some(502)
    );
    assert_eq!(
        rebuilt
            .relation_batch(relation_handle)
            .and_then(|value| value.source_rows().key(1)),
        Some(702)
    );
    assert_eq!(
        rebuilt
            .domain_visual(RowDomain::Points(point_handle))
            .map(VisualDescriptor::order),
        Some(7)
    );
}

#[test]
fn legacy_rehydration_rejects_missing_generic_sources_atomically() {
    let mut scene = Scene::new();
    let _handle = scene.add_point_batch(point_batch(90, &[1, 2]));
    let description = scene.describe();
    assert!(Scene::from_description(&description, empty_sources()).is_err());
}

fn point_batch(namespace: u64, keys: &[u64]) -> PointBatch {
    PointBatch::new(
        Arc::from([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
        SourceRows::keyed(SourceNamespace(namespace), Arc::from(keys))
            .unwrap_or_else(|error| panic!("point keys validate: {error}")),
        PointGlyph::Sphere,
        PointStyle::default(),
    )
    .unwrap_or_else(|error| panic!("points validate: {error}"))
}

fn instance_batch() -> InstanceBatch {
    let template = Arc::new(
        AnalyticTemplate::new(
            Arc::from([
                AnalyticSphere {
                    center: [0.0; 3],
                    radius: 1.0,
                },
                AnalyticSphere {
                    center: [1.5, 0.0, 0.0],
                    radius: 0.5,
                },
            ]),
            Arc::from([]),
            SourceRows::keyed(SourceNamespace(50), Arc::from([501, 502]))
                .unwrap_or_else(|error| panic!("template keys validate: {error}")),
        )
        .unwrap_or_else(|error| panic!("template validates: {error}")),
    );
    InstanceBatch::new(
        template,
        Arc::from([
            RigidInstance::new(Vec3::ZERO, Quat::IDENTITY, 1.0)
                .unwrap_or_else(|error| panic!("transform validates: {error}")),
            RigidInstance::new(Vec3::splat(4.0), Quat::IDENTITY, 2.0)
                .unwrap_or_else(|error| panic!("transform validates: {error}")),
        ]),
        SourceRows::keyed(SourceNamespace(60), Arc::from([601, 602]))
            .unwrap_or_else(|error| panic!("instance keys validate: {error}")),
    )
    .unwrap_or_else(|error| panic!("instances validate: {error}"))
}

fn relations(
    points: crate::PointBatchHandle,
    instances: crate::InstanceBatchHandle,
) -> RelationBatch {
    RelationBatch::new(
        Arc::from([
            Relation {
                start: SpatialAnchor::entity(RowEntityRef::new(RowDomain::Points(points), 1))
                    .unwrap_or_else(|error| panic!("point anchor validates: {error}")),
                end: SpatialAnchor::world(Vec3::Y)
                    .unwrap_or_else(|error| panic!("world anchor validates: {error}")),
            },
            Relation {
                start: SpatialAnchor::template_part(TemplatePartRef::new(instances, 1, 1)),
                end: SpatialAnchor::world(Vec3::X)
                    .unwrap_or_else(|error| panic!("world anchor validates: {error}")),
            },
        ]),
        SourceRows::keyed(SourceNamespace(70), Arc::from([701, 702]))
            .unwrap_or_else(|error| panic!("relation keys validate: {error}")),
        RelationStyle::default(),
    )
    .unwrap_or_else(|error| panic!("relations validate: {error}"))
}

const fn empty_sources<'a>() -> SceneDescriptionSources<'a> {
    SceneDescriptionSources {
        structures: &[],
        volumes: &[],
        segmentations: &[],
        atom_properties: &[],
        meshes: &[],
    }
}
