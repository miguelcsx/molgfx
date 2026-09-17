use super::*;
use crate::LodPolicy;
use crate::streaming::test_support::structure;
use molgfx_core::{
    AtomSelection, Particle, ParticleShape, Primitive, PrimitiveHandle, RepresentationKind, Scene,
    StructureHandle,
};
use molgfx_math::{Camera, Projection, Quat, Rgba8, Vec3};

fn camera(distance: f32, far: f32) -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, distance),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far,
        },
    }
}

fn selected(index: &LodIndex, camera: &Camera) -> LodFrame {
    let mut frame = LodFrame::default();
    index.select_into(camera, [800, 800], LodPolicy::default(), None, &mut frame);
    frame
}

fn foreign_particle(scene: &mut Scene, owner: StructureHandle) -> PrimitiveHandle {
    let particle = match Particle::new(
        owner,
        Vec3::new(8.0, 9.0, 10.0),
        Quat::IDENTITY,
        Vec3::splat(3.0),
        ParticleShape::Sphere,
        Rgba8::opaque(240, 20, 30),
        0.25,
    ) {
        Ok(value) => value,
        Err(error) => panic!("foreign particle validates: {error}"),
    };
    match scene.add_primitives(&[Primitive::particle(particle)]) {
        Ok(Some(handle)) => handle,
        Ok(None) => panic!("non-empty foreign batch returns one handle"),
        Err(error) => panic!("foreign particle stores: {error}"),
    }
}

#[test]
fn lod_scene_reuses_and_bounds_coarse_representations_across_frames() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("LOD scene fixture enters a scene: {error}"),
    };
    let Some((structure, _)) = scene.structures().next() else {
        panic!("LOD scene has one structure")
    };
    let index = LodIndex::from_scene(&scene);
    let frame = selected(&index, &camera(10_000.0, 20_000.0));
    assert!(
        frame
            .level(structure)
            .is_some_and(|level| level != LodLevel::Atom)
    );
    let mut lod_scene = LodScene::default();
    assert!(lod_scene.apply(&mut scene, &index, &frame).is_ok());
    assert!(lod_scene.apply(&mut scene, &index, &frame).is_ok());
    assert_eq!(lod_scene.primitive_count(), 1);
    assert_eq!(scene.primitives().count(), 1);

    let atom_frame = selected(&index, &camera(10.0, 100.0));
    assert_eq!(atom_frame.level(structure), Some(LodLevel::Atom));
    assert!(lod_scene.apply(&mut scene, &index, &atom_frame).is_ok());
    assert_eq!(lod_scene.primitive_count(), 0);
    assert_eq!(scene.primitives().count(), 0);
    assert!(lod_scene.apply(&mut scene, &index, &frame).is_ok());
    assert_eq!(lod_scene.primitive_count(), 1);
    assert_eq!(scene.primitives().count(), 1);
}

#[test]
fn lod_scene_crossfades_coarse_records_without_reallocating_them() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("LOD transition fixture enters a scene: {error}"),
    };
    let index = LodIndex::from_scene(&scene);
    let coarse = selected(&index, &camera(10_000.0, 20_000.0));
    let mut lod_scene = LodScene::default();
    assert!(
        lod_scene
            .apply_transition(&mut scene, &index, &LodFrame::default(), &coarse, 0.5)
            .is_ok()
    );
    let opacity = scene
        .primitives()
        .find_map(|(_, primitive)| match primitive {
            Primitive::Particle(value) => Some(value.opacity),
            _ => None,
        });
    assert_eq!(opacity, Some(0.41));
    assert_eq!(lod_scene.primitive_count(), 1);
    assert!(
        lod_scene
            .apply_transition(&mut scene, &index, &LodFrame::default(), &coarse, 1.0)
            .is_ok()
    );
    assert_eq!(lod_scene.primitive_count(), 1);
}

#[test]
fn lod_scene_crossfades_bound_atom_detail_automatically() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("detail LOD fixture enters a scene: {error}"),
    };
    let Some((structure, _)) = scene.structures().next() else {
        panic!("detail LOD fixture has one structure")
    };
    let selection = match scene.add_structure_selection(structure, AtomSelection::All) {
        Ok(value) => value,
        Err(error) => panic!("detail selection stores: {error}"),
    };
    let representation = match scene.represent(selection, RepresentationKind::Spacefill) {
        Ok(value) => value,
        Err(error) => panic!("detail representation stores: {error}"),
    };
    let coarse_representation = match scene.represent(selection, RepresentationKind::Surface) {
        Ok(value) => value,
        Err(error) => panic!("coarse representation stores: {error}"),
    };
    let index = LodIndex::from_scene(&scene);
    let coarse = selected(&index, &camera(10_000.0, 20_000.0));
    let atom = selected(&index, &camera(10.0, 100.0));
    let mut lod_scene = LodScene::default();
    assert!(
        lod_scene
            .bind_detail_representation(&scene, structure, representation)
            .is_ok()
    );
    assert_eq!(lod_scene.detail_representation_count(), 1);
    assert!(
        lod_scene
            .bind_coarse_representation(&scene, structure, coarse_representation)
            .is_ok()
    );
    assert_eq!(lod_scene.coarse_representation_count(), 1);

    assert!(
        lod_scene
            .apply_transition(&mut scene, &index, &atom, &coarse, 0.25)
            .is_ok()
    );
    let Some(detail) = scene.representation(representation) else {
        panic!("detail representation remains live")
    };
    assert!(detail.visible);
    assert!((detail.material.opacity - 0.75).abs() < 1.0e-6);
    let Some(coarse_value) = scene.representation(coarse_representation) else {
        panic!("coarse representation remains live")
    };
    assert!(coarse_value.visible);
    assert!((coarse_value.material.opacity - 0.25).abs() < 1.0e-6);

    assert!(lod_scene.apply(&mut scene, &index, &coarse).is_ok());
    let Some(detail) = scene.representation(representation) else {
        panic!("detail representation remains live")
    };
    assert!(!detail.visible);
    assert!(detail.material.opacity.abs() < 1.0e-6);
    let Some(coarse_value) = scene.representation(coarse_representation) else {
        panic!("coarse representation remains live")
    };
    assert!(coarse_value.visible);
    assert!((coarse_value.material.opacity - 1.0).abs() < 1.0e-6);
    assert_eq!(lod_scene.primitive_count(), 0);

    lod_scene.unbind_detail_representation(&mut scene, representation);
    let Some(detail) = scene.representation(representation) else {
        panic!("released detail representation remains live")
    };
    assert!(detail.visible);
    assert!((detail.material.opacity - 1.0).abs() < 1.0e-6);
    lod_scene.unbind_coarse_representation(&mut scene, coarse_representation);
    assert_eq!(lod_scene.coarse_representation_count(), 0);
}

#[test]
fn moving_a_lod_scene_does_not_touch_coincident_handles_in_another_scene() {
    let source = structure();
    let mut first = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("first LOD fixture enters a scene: {error}"),
    };
    let mut second = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("second LOD fixture enters a scene: {error}"),
    };
    assert_ne!(first.cache_identity(), second.cache_identity());
    let Some((first_owner, _)) = first.structures().next() else {
        panic!("first scene has one structure")
    };
    let Some((second_owner, _)) = second.structures().next() else {
        panic!("second scene has one structure")
    };
    assert_eq!(first_owner, second_owner);

    let first_index = LodIndex::from_scene(&first);
    let second_index = LodIndex::from_scene(&second);
    let first_frame = selected(&first_index, &camera(10_000.0, 20_000.0));
    let second_frame = selected(&second_index, &camera(10_000.0, 20_000.0));
    let mut lod_scene = LodScene::default();
    assert!(
        lod_scene
            .apply(&mut first, &first_index, &first_frame)
            .is_ok()
    );
    let Some((first_lod, _)) = first.primitives().next() else {
        panic!("first scene contains its LOD primitive")
    };

    let foreign = foreign_particle(&mut second, second_owner);
    assert_eq!(first_lod, foreign);
    let foreign_value = second.primitive(foreign).copied();
    assert!(
        lod_scene
            .apply_transition(
                &mut second,
                &second_index,
                &LodFrame::default(),
                &second_frame,
                1.0,
            )
            .is_ok()
    );

    assert_eq!(second.primitive(foreign).copied(), foreign_value);
    assert_eq!(second.primitives().count(), 2);
    assert_eq!(lod_scene.primitive_count(), 1);
}

#[test]
fn cloned_lod_scenes_start_without_shared_primitive_ownership() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("clone LOD fixture enters a scene: {error}"),
    };
    let index = LodIndex::from_scene(&scene);
    let coarse = selected(&index, &camera(10_000.0, 20_000.0));
    let atom = selected(&index, &camera(10.0, 100.0));
    let mut owner = LodScene::default();
    assert!(owner.apply(&mut scene, &index, &coarse).is_ok());
    assert_eq!(scene.primitives().count(), 1);

    let mut first_clone = owner.clone();
    let mut second_clone = owner.clone();
    assert_eq!(first_clone.primitive_count(), 0);
    assert_eq!(second_clone.primitive_count(), 0);
    assert!(first_clone.apply(&mut scene, &index, &coarse).is_ok());
    assert!(second_clone.apply(&mut scene, &index, &coarse).is_ok());
    assert_eq!(scene.primitives().count(), 3);
    assert!(first_clone.apply(&mut scene, &index, &atom).is_ok());
    assert!(second_clone.apply(&mut scene, &index, &atom).is_ok());

    assert_eq!(scene.primitives().count(), 1);
    assert_eq!(owner.primitive_count(), 1);
}
