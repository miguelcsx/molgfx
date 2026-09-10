use crate::{
    AnalyticSphere, AnalyticTemplate, ChunkId, DatasetId, EntityKind, GlobalPickIdentity,
    InstanceBatch, LogicalRow, PointBatch, PointGlyph, PointStyle, Relation, RelationBatch,
    RelationStyle, RigidInstance, RowDomain, RowEntityRef, Scene, SourceNamespace, SourceRows,
    SpatialAnchor,
};
use pdviewx_math::{Quat, Vec3};
use std::sync::Arc;

#[test]
fn stale_relation_dependencies_reject_the_whole_batch() {
    let mut scene = Scene::new();
    let points = PointBatch::new(
        Arc::from([[0.0; 3]]),
        SourceRows::ordered(SourceNamespace(1), 1),
        PointGlyph::Disc,
        PointStyle::default(),
    )
    .unwrap();
    let handle = scene.add_point_batch(points);
    let anchor = SpatialAnchor::entity(RowEntityRef::new(RowDomain::Points(handle), 0)).unwrap();
    scene.remove_point_batch(handle);
    let batch = RelationBatch::new(
        Arc::from([Relation {
            start: anchor,
            end: SpatialAnchor::world(Vec3::splat(1.0)).unwrap(),
        }]),
        SourceRows::ordered(SourceNamespace(2), 1),
        RelationStyle::default(),
    )
    .unwrap();

    assert!(scene.add_relation_batch(batch).is_err());
    assert_eq!(scene.relation_batches().count(), 0);
}

#[test]
fn flattened_template_part_pick_resolves_both_external_keys() {
    let template = AnalyticTemplate::new(
        Arc::from([
            AnalyticSphere {
                center: [0.0; 3],
                radius: 1.0,
            },
            AnalyticSphere {
                center: [1.0, 0.0, 0.0],
                radius: 0.5,
            },
        ]),
        Arc::from([]),
        SourceRows::keyed(SourceNamespace(81), Arc::from([700_u64, 900]))
            .unwrap_or_else(|error| panic!("{error}")),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let batch = InstanceBatch::new(
        Arc::new(template),
        Arc::from([
            RigidInstance::new(Vec3::ZERO, Quat::IDENTITY, 1.0)
                .unwrap_or_else(|error| panic!("{error}")),
            RigidInstance::new(Vec3::X, Quat::IDENTITY, 1.0)
                .unwrap_or_else(|error| panic!("{error}")),
        ]),
        SourceRows::keyed(SourceNamespace(80), Arc::from([11_u64, 22]))
            .unwrap_or_else(|error| panic!("{error}")),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut scene = Scene::new();
    let handle = scene.add_instance_batch(batch);
    let chunk = ChunkId::new(u64::from(handle.row()) | (u64::from(handle.generation()) << 32));
    let identity = GlobalPickIdentity::new(
        DatasetId::new(81),
        chunk,
        LogicalRow::new(3),
        EntityKind::TemplatePart,
    );
    let resolved = scene
        .resolve_template_part_pick(identity)
        .unwrap_or_else(|| panic!("template-part occurrence resolves"));
    assert_eq!(resolved.reference().instance_row(), 1);
    assert_eq!(resolved.reference().part_row(), 1);
    assert_eq!(resolved.instance_source_key(), 22);
    assert_eq!(resolved.part_source_key(), 900);
}
