use super::{AnalyticSphere, AnalyticTemplate, InstanceBatch, RigidInstance};
use crate::{SourceNamespace, SourceRows};
use molgfx_math::{Quat, Vec3};
use std::sync::Arc;

#[test]
fn rigid_instance_is_exactly_thirty_two_bytes() {
    assert_eq!(std::mem::size_of::<RigidInstance>(), 32);
    assert_eq!(std::mem::align_of::<RigidInstance>(), 16);
}

#[test]
fn one_template_is_shared_by_every_compact_transform() {
    let template = Arc::new(
        AnalyticTemplate::new(
            Arc::from([AnalyticSphere {
                center: [0.0; 3],
                radius: 1.0,
            }]),
            Arc::from([]),
            SourceRows::ordered(SourceNamespace(2), 1),
        )
        .unwrap(),
    );
    let transforms: Arc<[RigidInstance]> = Arc::from([
        RigidInstance::new(Vec3::ZERO, Quat::IDENTITY, 1.0).unwrap(),
        RigidInstance::new(Vec3::splat(4.0), Quat::IDENTITY, 2.0).unwrap(),
    ]);
    let batch = InstanceBatch::new(
        Arc::clone(&template),
        transforms,
        SourceRows::ordered(SourceNamespace(3), 2),
    )
    .unwrap();
    assert!(Arc::ptr_eq(batch.template(), &template));
    assert_eq!(batch.template().part_count(), 1);
}
