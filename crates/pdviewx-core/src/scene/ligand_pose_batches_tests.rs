use super::*;
use crate::fixture;
use pdviewx_math::{Quat, Rgba8, Vec3};

fn scene_with_owner() -> (Scene, StructureHandle) {
    let scene = match Scene::from_structure(&fixture::structure()) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    (scene, owner)
}

fn template_and_pose() -> (LicoriceTemplate, LigandPose) {
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO, Vec3::Z], &[[0, 1]]) {
        Ok(value) => value,
        Err(error) => panic!("template validates: {error}"),
    };
    let pose = match LigandPose::new(
        Vec3::new(4.0, 2.0, 1.0),
        Quat::from_rotation_x(0.5),
        Rgba8::opaque(20, 80, 220),
        0.8,
    ) {
        Ok(value) => value,
        Err(error) => panic!("pose validates: {error}"),
    };
    (template, pose)
}

#[test]
fn pose_batches_store_one_compact_slot_and_advance_one_revision() {
    let (mut scene, owner) = scene_with_owner();
    let (template, pose) = template_and_pose();
    let revision = scene.primitive_revision();
    let handle = match scene.add_licorice_poses(owner, template, vec![pose, pose]) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("non-empty batch returns a handle"),
        Err(error) => panic!("pose batch stores: {error}"),
    };

    assert_eq!(scene.primitives().count(), 0);
    assert_eq!(scene.ligand_pose_batches().count(), 1);
    assert_eq!(
        scene
            .ligand_pose_batch(handle)
            .map(LigandPoseBatch::pose_count),
        Some(2)
    );
    assert_eq!(
        scene
            .ligand_pose_batch(handle)
            .map(LigandPoseBatch::instance_count),
        Some(6)
    );
    assert_eq!(scene.primitive_revision(), revision.wrapping_add(1));
}

#[test]
fn pose_batch_visibility_and_removal_preserve_handle_semantics() {
    let (mut scene, owner) = scene_with_owner();
    let (template, pose) = template_and_pose();
    let handle = match scene.add_licorice_poses(owner, template, vec![pose]) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("non-empty batch returns a handle"),
        Err(error) => panic!("pose batch stores: {error}"),
    };
    let revision = scene.primitive_revision();

    assert!(scene.hide_ligand_pose_batch(handle));
    assert!(!scene.hide_ligand_pose_batch(handle));
    assert_eq!(scene.primitive_revision(), revision.wrapping_add(1));
    assert!(scene.show_ligand_pose_batch(handle));
    assert!(scene.remove_ligand_pose_batch(handle).is_some());
    assert!(scene.ligand_pose_batch(handle).is_none());
    assert!(!scene.show_ligand_pose_batch(handle));
}

#[test]
fn reused_pose_batch_rows_advance_the_generation() {
    let (mut scene, owner) = scene_with_owner();
    let (template, pose) = template_and_pose();
    let first = match scene.add_licorice_poses(owner, template.clone(), vec![pose]) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("non-empty batch returns a handle"),
        Err(error) => panic!("pose batch stores: {error}"),
    };
    assert!(scene.remove_ligand_pose_batch(first).is_some());
    let second = match scene.add_licorice_poses(owner, template, vec![pose]) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("non-empty batch returns a handle"),
        Err(error) => panic!("replacement pose batch stores: {error}"),
    };

    assert_eq!(second.row(), first.row());
    assert_eq!(second.generation(), first.generation().wrapping_add(1));
    assert!(scene.ligand_pose_batch(first).is_none());
    assert!(scene.ligand_pose_batch(second).is_some());
}

#[test]
fn hidden_pose_batches_do_not_expand_the_scene_bound() {
    let (mut scene, owner) = scene_with_owner();
    let (template, _) = template_and_pose();
    let pose = match LigandPose::new(Vec3::splat(1_000.0), Quat::IDENTITY, Rgba8::WHITE, 1.0) {
        Ok(value) => value,
        Err(error) => panic!("far pose validates: {error}"),
    };
    let handle = match scene.add_licorice_poses(owner, template, vec![pose]) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("non-empty batch returns a handle"),
        Err(error) => panic!("pose batch stores: {error}"),
    };
    assert!(scene.world_aabb().max.x > 900.0);
    assert!(scene.hide_ligand_pose_batch(handle));
    assert!(scene.world_aabb().max.x < 900.0);
}

#[test]
fn every_generated_row_resolves_to_batch_level_provenance() {
    let (mut scene, owner) = scene_with_owner();
    let (template, pose) = template_and_pose();
    let handle = match scene.add_licorice_poses(owner, template, vec![pose]) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("non-empty batch returns a handle"),
        Err(error) => panic!("pose batch stores: {error}"),
    };
    let entity = EntityRef {
        structure: owner,
        kind: EntityKind::LigandPoseBatch,
        index: Scene::ligand_pose_batch_entity_row(handle),
    };

    assert!(scene.ligand_pose_batch_for_entity(entity).is_some());
    assert!(scene.primitive_for_entity(entity).is_none());
}

#[test]
fn pose_batches_reject_stale_owners_atomically() {
    let (mut scene, owner) = scene_with_owner();
    let (template, pose) = template_and_pose();
    assert!(scene.remove_structure(owner).is_some());

    assert!(matches!(
        scene.add_licorice_poses(owner, template, vec![pose]),
        Err(CoreError::StaleHandle)
    ));
    assert_eq!(scene.ligand_pose_batches().count(), 0);
}

#[test]
fn pose_batches_reject_instance_counts_outside_one_gpu_draw() {
    let (mut scene, owner) = scene_with_owner();
    let atoms = vec![Vec3::ZERO; 65_536];
    let template = match LicoriceTemplate::new(Vec3::ZERO, atoms, &[]) {
        Ok(value) => value,
        Err(error) => panic!("large template validates: {error}"),
    };
    let (_, pose) = template_and_pose();
    let poses = vec![pose; 65_536];

    assert!(matches!(
        scene.add_licorice_poses(owner, template, poses),
        Err(CoreError::InvalidPrimitive { .. })
    ));
    assert_eq!(scene.ligand_pose_batches().count(), 0);
}
