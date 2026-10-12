use super::*;
use crate::{
    Relation, RelationStyle, RowEntityRef, SourceNamespace, SourceRows, TrajectoryFrame,
    TrajectorySegment,
};
use std::sync::Arc;

#[test]
fn scientific_relation_samples_and_intervals_expire_without_changing_or_copying_their_rows() {
    let source = crate::fixture::structure();
    let mut scene = Scene::from_structure(&source).expect("source");
    let owner = scene.structures().next().expect("owner").0;
    let positions: Arc<[[f32; 3]]> = Arc::from(source.coordinates());
    let interval = TrajectorySegment::new(
        TrajectoryFrame::new(0, 0.0, positions.clone(), "start").expect("start"),
        TrajectoryFrame::new(1, 1.0, positions.clone(), "end").expect("end"),
        0.0,
    )
    .expect("interval");
    scene
        .set_trajectory_segment(owner, interval.clone())
        .expect("bind");
    let pose = MolecularPose::capture(scene.structure(owner).expect("placed"));
    let domain = RowDomain::Atoms(owner);
    let rows: Arc<[Relation]> = Arc::from([Relation {
        start: SpatialAnchor::entity(RowEntityRef::new(domain, 0)).expect("first"),
        end: SpatialAnchor::entity(RowEntityRef::new(domain, 1)).expect("second"),
    }]);
    let table = RelationBatch::new(
        rows.clone(),
        SourceRows::ordered(SourceNamespace(91), 1),
        RelationStyle::default(),
    )
    .expect("table");
    let sample = table
        .clone()
        .with_molecular_sample(pose.clone())
        .expect("sample");
    let candidates = table.with_molecular_interval(pose).expect("candidates");
    assert!(Arc::ptr_eq(sample.relations(), &rows));
    assert!(Arc::ptr_eq(candidates.relations(), &rows));
    let sample = scene.add_relation_batch(sample).expect("sample binds");
    let candidates = scene
        .add_relation_batch(candidates)
        .expect("candidates bind");
    let current = |scene: &Scene, handle| {
        scene
            .relation_batch(handle)
            .expect("table")
            .molecular_coordinates()
            .expect("input")
            .matches(scene)
    };
    scene
        .set_trajectory_time(owner, 0.5)
        .expect("sample advances");
    assert!(!current(&scene, sample));
    assert!(current(&scene, candidates));
    let replacement = TrajectorySegment::new(
        TrajectoryFrame::new(0, 0.0, Arc::from(source.coordinates()), "different owner")
            .expect("first"),
        TrajectoryFrame::new(1, 1.0, positions, "end").expect("second"),
        0.0,
    )
    .expect("replacement");
    scene
        .set_trajectory_segment(owner, replacement)
        .expect("pair advances");
    assert!(!current(&scene, sample));
    assert!(!current(&scene, candidates));
    scene
        .set_trajectory_segment(owner, interval)
        .expect("original pair returns");
    assert!(current(&scene, sample));
    assert!(current(&scene, candidates));
    let foreign = Scene::from_structure(&crate::fixture::structure()).expect("foreign");
    let foreign_pose =
        MolecularPose::capture(foreign.structures().next().expect("foreign placed").1);
    let invalid = scene
        .relation_batch(candidates)
        .expect("original")
        .clone()
        .with_molecular_interval(foreign_pose)
        .expect("foreign table");
    assert!(scene.add_relation_batch(invalid).is_err());
    assert_eq!(scene.relation_batches().count(), 2);
}
