use super::*;

#[test]
fn a_cloud_retains_compact_positions_and_rejects_nonfinite_rows() {
    let positions: Arc<[[f32; 3]]> = Arc::from([[0.0; 3], [1.0; 3]]);
    let cloud = PointCloud::new(
        vec![(Arc::clone(&positions), Color::rgb(20, 80, 120))],
        0.05,
    )
    .expect("finite cloud");
    assert_eq!(cloud.point_count(), 2);
    let (_, batch) = cloud.scene.point_batches().next().expect("one group");
    assert_eq!(batch.sampling(), PointSampling::All);
    assert!(Arc::ptr_eq(batch.positions(), &positions));
    assert!(PointCloud::new(vec![], 0.05).is_err());
    assert!(
        PointCloud::new(
            vec![(Arc::from([[f32::NAN; 3]]), Color::rgb(0, 0, 0))],
            0.05
        )
        .is_err()
    );
}

#[test]
fn composition_keeps_molecular_representations_and_shared_point_storage() {
    let pdb =
        b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00           C\nEND\n";
    let (structure, _) =
        molframe::read_bytes(pdb.to_vec(), Some("one.pdb"), &molframe::ReadOptions::new())
            .expect("fixture");
    let mut scene = crate::Scene::from_structure(&structure).expect("molecular scene");
    scene
        .add(crate::rep::spacefill("all"))
        .expect("representation");
    let positions: Arc<[[f32; 3]]> = Arc::from([[2.0, 0.0, 0.0]]);
    let cloud = PointCloud::new(vec![(Arc::clone(&positions), Color::rgb(0, 80, 120))], 0.05)
        .expect("cloud");
    let composed = cloud.with_scene(&scene).expect("composition");
    assert_eq!(composed.scene.structures().count(), 1);
    assert_eq!(composed.scene.representations().count(), 1);
    assert_eq!(composed.point_count(), 1);
    let (_, batch) = composed
        .scene
        .point_batches()
        .next()
        .expect("shared points");
    assert!(Arc::ptr_eq(batch.positions(), &positions));
    assert_eq!(cloud.scene.structures().count(), 0);
}
