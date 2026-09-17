use super::super::*;
use crate::fixture;
use molgfx_math::{Mat4, Rgba8, Vec3};

fn scene_with_owner() -> (Scene, StructureHandle) {
    let scene = match Scene::from_structure(&fixture::structure()) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has an owner")
    };
    (scene, owner)
}

#[test]
fn overlay_handles_edit_and_reject_stale_generations() {
    let (mut scene, _) = scene_with_owner();
    let anchor = match OverlayAnchor::new([0.1, 0.9], [8.0, -8.0]) {
        Ok(value) => value,
        Err(error) => panic!("anchor validates: {error}"),
    };
    let overlay = match ScreenOverlay::new(
        OverlayContent::Text {
            text: "ACTIVE SITE".to_owned(),
            color: Rgba8::WHITE,
            size_pixels: 14.0,
        },
        anchor,
    ) {
        Ok(value) => value,
        Err(error) => panic!("overlay validates: {error}"),
    };
    let revision = scene.overlay_revision();
    let handle = scene.add_overlay(overlay);
    assert!(scene.overlay_revision() > revision);
    let Some(value) = scene.overlay_mut(handle) else {
        panic!("overlay resolves")
    };
    value.set_order(7);
    assert_eq!(scene.overlay(handle).map(ScreenOverlay::order), Some(7));
    assert!(scene.remove_overlay(handle).is_some());
    assert!(scene.overlay(handle).is_none());
}

#[test]
fn mesh_instances_share_one_source_and_keep_stable_handles() {
    let (mut scene, owner) = scene_with_owner();
    let vertices = vec![
        MeshVertex {
            position: Vec3::ZERO,
            normal: Vec3::Z,
            color: Rgba8::WHITE,
        },
        MeshVertex {
            position: Vec3::X,
            normal: Vec3::Z,
            color: Rgba8::WHITE,
        },
        MeshVertex {
            position: Vec3::Y,
            normal: Vec3::Z,
            color: Rgba8::WHITE,
        },
    ];
    let mesh = match Mesh::new(owner, vertices, vec![0, 1, 2], Material::default()) {
        Ok(value) => value,
        Err(error) => panic!("mesh validates: {error}"),
    };
    let mesh = match scene.add_mesh(mesh) {
        Ok(value) => value,
        Err(error) => panic!("mesh stores: {error}"),
    };
    let instance = match MeshInstance::new(mesh, Mat4::from_translation(Vec3::X * 4.0)) {
        Ok(value) => value,
        Err(error) => panic!("instance validates: {error}"),
    };
    let handle = match scene.add_mesh_instance(instance) {
        Ok(value) => value,
        Err(error) => panic!("instance stores: {error}"),
    };
    assert_eq!(scene.mesh_instances().count(), 1);
    assert_eq!(scene.mesh_instance_transforms(mesh).count(), 1);
    assert!(scene.remove_mesh_instance(handle).is_some());
    assert!(scene.mesh_instance(handle).is_none());
}
