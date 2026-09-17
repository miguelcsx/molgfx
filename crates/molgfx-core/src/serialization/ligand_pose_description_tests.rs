use super::*;
use crate::{
    Annotation, AnnotationAnchor, EntityKind, EntityRef, ProvenanceDetail, SceneDescriptionSources,
    fixture,
};

#[test]
fn compact_pose_batches_round_trip_with_stable_identity_and_visibility() {
    let structure = fixture::structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO, Vec3::X], &[[0, 1]]) {
        Ok(value) => value,
        Err(error) => panic!("template validates: {error}"),
    };
    let pose = match LigandPose::new(
        Vec3::new(3.0, 2.0, 1.0),
        Quat::from_rotation_y(0.4),
        Rgba8::opaque(30, 140, 220),
        0.7,
    ) {
        Ok(value) => value,
        Err(error) => panic!("pose validates: {error}"),
    };
    let handle = match scene.add_licorice_poses(owner, template, vec![pose, pose]) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("non-empty pose batch returns a handle"),
        Err(error) => panic!("pose batch stores: {error}"),
    };
    assert!(scene.hide_ligand_pose_batch(handle));
    let batch_entity = EntityRef {
        structure: owner,
        kind: EntityKind::LigandPoseBatch,
        index: Scene::ligand_pose_batch_entity_row(handle),
    };
    let anchor = match AnnotationAnchor::entity(Vec3::ZERO, batch_entity) {
        Ok(value) => value,
        Err(error) => panic!("batch anchor validates: {error}"),
    };
    let annotation = match Annotation::note(owner, anchor, "candidate batch") {
        Ok(value) => value,
        Err(error) => panic!("batch annotation validates: {error}"),
    };
    if let Err(error) = scene.add_annotation(annotation) {
        panic!("batch annotation stores: {error}")
    }

    let description = scene.describe();
    let sources = SceneDescriptionSources {
        structures: std::slice::from_ref(&structure),
        volumes: &[],
        segmentations: &[],
        atom_properties: &[],
        meshes: &[],
    };
    let restored = match Scene::from_description(&description, sources) {
        Ok(value) => value,
        Err(error) => panic!("pose batch manifest restores: {error}"),
    };

    let Some((restored_handle, batch)) = restored.ligand_pose_batches().next() else {
        panic!("restored pose batch exists")
    };
    assert_eq!(restored_handle.row(), handle.row());
    assert_eq!(restored_handle.generation(), handle.generation());
    assert_eq!(batch.pose_count(), 2);
    assert_eq!(batch.instance_count(), 6);
    assert!(!batch.visible());
    let restored_entity = EntityRef {
        structure: batch.owner(),
        kind: EntityKind::LigandPoseBatch,
        index: restored_handle.row(),
    };
    assert!(matches!(
        restored
            .provenance(restored_entity)
            .map(|value| value.detail),
        Some(ProvenanceDetail::LigandPoseBatch(_))
    ));
    assert_eq!(restored.describe(), description);
}

#[test]
fn manifests_reject_unpickable_or_empty_pose_batches() {
    let structure = fixture::structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO], &[]) {
        Ok(value) => value,
        Err(error) => panic!("template validates: {error}"),
    };
    let pose = match LigandPose::new(Vec3::ZERO, Quat::IDENTITY, Rgba8::WHITE, 1.0) {
        Ok(value) => value,
        Err(error) => panic!("pose validates: {error}"),
    };
    if let Err(error) = scene.add_licorice_poses(owner, template, vec![pose]) {
        panic!("pose batch stores: {error}")
    }
    let mut description = scene.describe();
    let sources = SceneDescriptionSources {
        structures: std::slice::from_ref(&structure),
        volumes: &[],
        segmentations: &[],
        atom_properties: &[],
        meshes: &[],
    };

    let Some(batch) = description.ligand_pose_batches.first_mut() else {
        panic!("manifest contains the pose batch")
    };
    let original_row = batch.row;
    batch.row = crate::EntityId::MAX_INDEX + 1;
    assert!(Scene::from_description(&description, sources).is_err());
    let Some(batch) = description.ligand_pose_batches.first_mut() else {
        panic!("manifest still contains the pose batch")
    };
    batch.row = original_row;
    description.tables.ligand_pose_batches = 0;
    assert!(Scene::from_description(&description, sources).is_err());
    description.tables.ligand_pose_batches = 1;
    let Some(batch) = description.ligand_pose_batches.first_mut() else {
        panic!("manifest still contains the pose batch")
    };
    batch.poses.clear();
    assert!(Scene::from_description(&description, sources).is_err());
}

#[test]
fn manifest_rejects_the_reserved_pose_batch_row_before_rehydration() {
    let structure = fixture::structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO], &[]) {
        Ok(value) => value,
        Err(error) => panic!("template validates: {error}"),
    };
    let pose = match LigandPose::new(Vec3::ZERO, Quat::IDENTITY, Rgba8::WHITE, 1.0) {
        Ok(value) => value,
        Err(error) => panic!("pose validates: {error}"),
    };
    if let Err(error) = scene.add_licorice_poses(owner, template, vec![pose]) {
        panic!("pose batch stores: {error}")
    }
    let mut description = scene.describe();
    let Some(batch) = description.ligand_pose_batches.first_mut() else {
        panic!("manifest contains the pose batch")
    };
    batch.row = u32::MAX;
    let result = Scene::from_description(
        &description,
        SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    );
    assert!(matches!(
        result,
        Err(crate::CoreError::InvalidSceneDescription { summary })
            if summary.contains("identity range")
    ));
}

#[test]
fn sparse_pose_batch_rows_and_generations_round_trip() {
    let structure = fixture::structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO], &[]) {
        Ok(value) => value,
        Err(error) => panic!("template validates: {error}"),
    };
    let pose = match LigandPose::new(Vec3::ZERO, Quat::IDENTITY, Rgba8::WHITE, 1.0) {
        Ok(value) => value,
        Err(error) => panic!("pose validates: {error}"),
    };
    if let Err(error) = scene.add_licorice_poses(owner, template, vec![pose]) {
        panic!("pose batch stores: {error}")
    }
    let mut description = scene.describe();
    let Some(batch) = description.ligand_pose_batches.first_mut() else {
        panic!("manifest contains the pose batch")
    };
    batch.row = crate::EntityId::MAX_INDEX;
    batch.generation = 11;
    let sources = SceneDescriptionSources {
        structures: std::slice::from_ref(&structure),
        volumes: &[],
        segmentations: &[],
        atom_properties: &[],
        meshes: &[],
    };
    let restored = match Scene::from_description(&description, sources) {
        Ok(value) => value,
        Err(error) => panic!("sparse pose batch restores: {error}"),
    };
    let Some((handle, _)) = restored.ligand_pose_batches().next() else {
        panic!("restored pose batch exists")
    };
    assert_eq!(handle.row(), crate::EntityId::MAX_INDEX);
    assert_eq!(handle.generation(), 11);
    assert_eq!(restored.describe(), description);
}
