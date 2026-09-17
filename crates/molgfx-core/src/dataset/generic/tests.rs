use super::*;
use crate::{
    AttributeValues, ChunkDomainKind, ChunkDomainRef, ChunkId, ChunkOccurrenceId, DatasetId,
    LogicalRow, RigidInstance,
};
use molgfx_math::{Quat, Vec3};
use std::sync::Arc;

#[test]
fn generic_payloads_retain_native_storage_without_scene_handles() {
    let positions: Arc<[[f32; 3]]> = Arc::from([[1.0, 2.0, 3.0]]);
    let pointer = positions.as_ptr();
    let points = PointChunkPayload::new(positions)
        .unwrap_or_else(|error| panic!("points validate: {error}"));
    assert_eq!(points.positions().as_ptr(), pointer);

    let transforms: Arc<[RigidInstance]> =
        Arc::from([RigidInstance::new(Vec3::ZERO, Quat::IDENTITY, 1.0)
            .unwrap_or_else(|error| panic!("transform validates: {error}"))]);
    let pointer = transforms.as_ptr();
    let instances = InstanceChunkPayload::new(transforms)
        .unwrap_or_else(|error| panic!("instances validate: {error}"));
    assert_eq!(instances.transforms().as_ptr(), pointer);

    let values: Arc<[[f32; 3]]> = Arc::from([[0.1, 0.2, 0.3]]);
    let pointer = values.as_ptr();
    let attribute = AttributeChunkPayload::new(
        ChunkDomainRef::new(DatasetId::new(7), ChunkId::new(11), ChunkDomainKind::Point),
        AttributeValues::Vector(values),
    )
    .unwrap_or_else(|error| panic!("attribute validates: {error}"));
    let AttributeValues::Vector(values) = attribute.values() else {
        panic!("vector values expected")
    };
    assert_eq!(values.as_ptr(), pointer);
}

#[test]
fn relation_payloads_use_global_chunk_identity_and_reject_bad_template_sources() {
    let point = ChunkEntityRef::new(
        DatasetId::new(7),
        ChunkId::new(11),
        ChunkOccurrenceId::new(19),
        LogicalRow::new(u64::from(u32::MAX) + 5),
        ChunkSpatialKind::Point,
    );
    assert_eq!(point.occurrence(), ChunkOccurrenceId::new(19));
    let relations = RelationChunkPayload::new(Arc::from([PagedRelation {
        start: PagedSpatialAnchor::World(Vec3::ZERO),
        end: PagedSpatialAnchor::Entity(point),
    }]))
    .unwrap_or_else(|error| panic!("relations validate: {error}"));
    assert_eq!(
        relations.relations()[0].end,
        PagedSpatialAnchor::Entity(point)
    );

    let invalid = TemplatePartChunkRef::new(point, 0);
    assert!(
        RelationChunkPayload::new(Arc::from([PagedRelation {
            start: PagedSpatialAnchor::World(Vec3::ZERO),
            end: PagedSpatialAnchor::TemplatePart(invalid),
        }]))
        .is_err()
    );
}
