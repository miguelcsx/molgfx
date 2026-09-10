use super::{AnchorLayout, Relation, RelationBatch, RelationStyle, SpatialAnchor};
use crate::{
    InstanceBatchHandle, PointBatchHandle, RowDomain, RowEntityRef, SourceNamespace, SourceRows,
    TemplatePartRef, handle::RawHandle,
};
use pdviewx_math::Vec3;
use std::sync::Arc;

fn points() -> RowDomain {
    RowDomain::Points(PointBatchHandle(RawHandle::new_for_test(4, 1)))
}

#[test]
fn mixed_relations_partition_stably_and_keep_a_remap_only_when_needed() {
    let dynamic = SpatialAnchor::entity(RowEntityRef::new(points(), 0)).unwrap();
    let world_a = SpatialAnchor::world(Vec3::ZERO).unwrap();
    let world_b = SpatialAnchor::world(Vec3::splat(1.0)).unwrap();
    let relations: Arc<[Relation]> = Arc::from([
        Relation {
            start: dynamic,
            end: world_b,
        },
        Relation {
            start: world_a,
            end: world_b,
        },
        Relation {
            start: dynamic,
            end: world_a,
        },
    ]);
    let batch = RelationBatch::new(
        relations,
        SourceRows::ordered(SourceNamespace(1), 3),
        RelationStyle::default(),
    )
    .unwrap();

    assert_eq!(batch.partitions().len(), 2);
    assert_eq!(batch.partitions()[0].layout.start, AnchorLayout::World);
    assert_eq!(
        batch.remap().map(std::convert::AsRef::as_ref),
        Some(&[1, 0, 2][..])
    );
}

#[test]
fn relations_cannot_anchor_to_relations() {
    let relation_domain =
        RowDomain::Relations(crate::RelationBatchHandle(RawHandle::new_for_test(1, 0)));
    assert!(SpatialAnchor::entity(RowEntityRef::new(relation_domain, 0)).is_err());
}

#[test]
fn template_part_anchors_require_an_exact_instance_occurrence() {
    let batch = InstanceBatchHandle(RawHandle::new_for_test(8, 2));
    let ambiguous = RowEntityRef::new(RowDomain::TemplateParts(batch), 3);
    assert!(SpatialAnchor::entity(ambiguous).is_err());

    let exact = SpatialAnchor::template_part(TemplatePartRef::new(batch, 5, 3));
    assert_eq!(exact.layout(), AnchorLayout::TemplatePart);
    assert!(exact.is_dynamic());
    let relations = Arc::from([Relation {
        start: exact,
        end: SpatialAnchor::world(Vec3::ZERO).unwrap(),
    }]);
    let batch = RelationBatch::new(
        relations,
        SourceRows::ordered(SourceNamespace(2), 1),
        RelationStyle::default(),
    )
    .unwrap();
    assert_eq!(batch.dependencies().len(), 2);
    assert_eq!(batch.dependencies()[0].maximum_row, 5);
    assert_eq!(batch.dependencies()[1].maximum_row, 3);
}
