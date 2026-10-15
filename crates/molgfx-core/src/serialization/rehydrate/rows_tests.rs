use super::*;

#[test]
fn every_rehydrated_identity_table_rejects_the_reserved_row() {
    let mut scene = Scene::new();
    assert!(prepare_table(&mut scene.structures, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.selections, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.properties, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.representations, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.volumes, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.segmentations, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.meshes, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.mesh_instances, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.primitive, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.ligand_pose_batches, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.overlays, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.guides, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.interactions, [u32::MAX].into_iter()).is_err());
    assert!(prepare_table(&mut scene.labels, [u32::MAX].into_iter()).is_err());
}

#[test]
fn every_pickable_table_rejects_rows_that_would_alias_on_the_gpu() {
    let mut scene = Scene::new();
    let row = crate::EntityId::MAX_INDEX + 1;
    assert!(prepare_pickable_table(&mut scene.meshes, [row].into_iter()).is_err());
    assert!(prepare_pickable_table(&mut scene.primitive, [row].into_iter()).is_err());
    assert!(prepare_pickable_table(&mut scene.ligand_pose_batches, [row].into_iter()).is_err());
    assert!(prepare_pickable_table(&mut scene.guides, [row].into_iter()).is_err());
    assert!(prepare_pickable_table(&mut scene.interactions, [row].into_iter()).is_err());
    assert!(prepare_pickable_table(&mut scene.labels, [row].into_iter()).is_err());
}
