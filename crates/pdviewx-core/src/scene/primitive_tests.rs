use super::super::*;
use crate::fixture;
use pdviewx_math::{Aabb, Quat, Rgba8, Vec3};

fn scene_with_owner() -> (Scene, StructureHandle) {
    let scene = match Scene::from_structure(&fixture::structure()) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    (scene, owner)
}

#[test]
fn primitives_share_revisions_bounds_and_pick_provenance() {
    let (mut scene, owner) = scene_with_owner();
    let initial = scene.primitive_revision();
    let ellipsoid = match AnisotropicEllipsoid::new(Vec3::ZERO, [4.0, 1.0, 1.0, 0.0, 0.0, 0.0]) {
        Ok(value) => value,
        Err(error) => panic!("ellipsoid validates: {error}"),
    };
    let symbol = match CarbohydrateSymbol::new(
        owner,
        Vec3::new(4.0, 0.0, 0.0),
        Quat::IDENTITY,
        Vec3::splat(2.0),
        CarbohydrateShape::Glc,
        Rgba8::opaque(20, 120, 220),
    ) {
        Ok(value) => value,
        Err(error) => panic!("symbol validates: {error}"),
    };
    let plane = match PlanarRegion::new(
        owner,
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::Z,
        Vec3::X,
        [4.0, 2.0],
    ) {
        Ok(value) => value,
        Err(error) => panic!("plane validates: {error}"),
    };
    let values = [
        match Primitive::ellipsoid(owner, ellipsoid, Rgba8::WHITE, 1.0) {
            Ok(value) => value,
            Err(error) => panic!("ellipsoid declares: {error}"),
        },
        Primitive::carbohydrate(symbol),
        match Primitive::planar(plane, Rgba8::opaque(220, 80, 40), 0.5) {
            Ok(value) => value,
            Err(error) => panic!("plane declares: {error}"),
        },
    ];
    if let Err(error) = scene.add_primitives(&values) {
        panic!("primitive batch stores: {error}")
    }
    let handles = scene
        .primitives()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    let [ellipsoid_handle, symbol_handle, plane_handle] = handles.as_slice() else {
        panic!("three handles resolve")
    };
    let (ellipsoid_handle, symbol_handle, plane_handle) =
        (*ellipsoid_handle, *symbol_handle, *plane_handle);

    assert!(scene.primitive_revision() > initial);
    assert_eq!(scene.primitives().count(), 3);
    assert!(scene.world_aabb().max.z >= 3.0);
    let entity = EntityRef {
        structure: owner,
        kind: EntityKind::Primitive,
        index: Scene::primitive_row(ellipsoid_handle),
    };
    let Some(provenance) = scene.provenance(entity) else {
        panic!("primitive provenance resolves")
    };
    assert!(matches!(provenance.detail, ProvenanceDetail::Primitive(_)));

    let revision = scene.primitive_revision();
    let Some(Primitive::Planar { visible, .. }) = scene.primitive_mut(plane_handle) else {
        panic!("plane resolves mutably")
    };
    *visible = false;
    assert!(scene.primitive_revision() > revision);
    assert!(scene.primitive(symbol_handle).is_some());
    assert!(scene.remove_primitive(ellipsoid_handle).is_some());
    assert!(scene.primitive(ellipsoid_handle).is_none());
}

#[test]
fn primitives_reject_stale_owners_and_invalid_opacity() {
    let (mut scene, owner) = scene_with_owner();
    let value = match AnisotropicEllipsoid::new(Vec3::ZERO, [1.0, 1.0, 1.0, 0.0, 0.0, 0.0]) {
        Ok(value) => value,
        Err(error) => panic!("ellipsoid validates: {error}"),
    };
    let stale = match scene.add_structure(&fixture::structure()) {
        Ok(handle) => handle,
        Err(error) => panic!("second fixture places: {error}"),
    };
    assert!(scene.remove_structure(stale).is_some());
    assert!(matches!(
        scene.add_primitives(&[
            match Primitive::ellipsoid(stale, value, Rgba8::WHITE, 1.0) {
                Ok(value) => value,
                Err(error) => panic!("stale primitive still declares: {error}"),
            }
        ]),
        Err(CoreError::StaleHandle)
    ));
    assert!(matches!(
        Primitive::ellipsoid(owner, value, Rgba8::WHITE, f32::NAN),
        Err(CoreError::InvalidPrimitive { .. })
    ));
}

#[test]
fn particles_and_streamlines_lower_to_pickable_scene_tables() {
    let (mut scene, owner) = scene_with_owner();
    let motion = match ParticleMotion::new(
        Vec3::X,
        Aabb::new(Vec3::splat(-8.0), Vec3::splat(8.0)),
        1.0 / 60.0,
        7,
        ParticleBoundary::Bounce,
    ) {
        Ok(value) => value,
        Err(error) => panic!("particle motion validates: {error}"),
    };
    let particle = match Particle::new(
        owner,
        Vec3::new(2.0, 0.0, 0.0),
        Quat::IDENTITY,
        Vec3::splat(1.5),
        ParticleShape::Sphere,
        Rgba8::opaque(60, 180, 240),
        0.75,
    ) {
        Ok(value) => value.with_motion(motion),
        Err(error) => panic!("particle validates: {error}"),
    };
    let primitive = match scene.add_primitives(&[Primitive::particle(particle)]) {
        Ok(Some(handle)) => handle,
        Ok(None) => panic!("non-empty batch returns its last handle"),
        Err(error) => panic!("particle stores: {error}"),
    };
    let points = [
        Vec3::new(-2.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
    ];
    let guides = match scene.add_streamline(owner, &points, GuideStyle::default()) {
        Ok(handles) => handles,
        Err(error) => panic!("streamline stores: {error}"),
    };
    assert_eq!(guides.len(), 2);
    assert_eq!(scene.guides().count(), guides.len());
    assert!(scene.primitive(primitive).is_some());
    assert!(scene.world_aabb().min.x <= -8.0);
    assert!(scene.world_aabb().max.x >= 8.0);
    assert!(matches!(
        scene.add_streamline(owner, &[Vec3::ZERO], GuideStyle::default()),
        Err(CoreError::InvalidAnnotation { .. })
    ));

    let bundle = vec![
        vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0],
        vec![Vec3::Y, Vec3::Y + Vec3::X],
    ];
    let handles = match scene.add_streamline_bundle(owner, &bundle, GuideStyle::default()) {
        Ok(value) => value,
        Err(error) => panic!("streamline bundle stores: {error}"),
    };
    assert_eq!(handles.iter().map(Vec::len).sum::<usize>(), 3);
    assert!(matches!(
        scene.add_streamline_bundle(owner, &[vec![Vec3::ZERO]], GuideStyle::default()),
        Err(CoreError::InvalidAnnotation { .. })
    ));
}

#[test]
fn one_native_batch_validates_atomically_and_advances_one_revision() {
    let (mut scene, owner) = scene_with_owner();
    let particle = match Particle::new(
        owner,
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::ONE,
        ParticleShape::Sphere,
        Rgba8::WHITE,
        1.0,
    ) {
        Ok(value) => value,
        Err(error) => panic!("particle builds: {error}"),
    };
    let revision = scene.primitive_revision();
    assert!(
        scene
            .add_primitives(&[Primitive::Particle(particle), Primitive::Particle(particle)])
            .is_ok()
    );
    assert_eq!(scene.primitives().count(), 2);
    assert_eq!(scene.primitive_revision(), revision.wrapping_add(1));
}

#[test]
fn closed_polylines_add_one_closing_segment() {
    let (mut scene, owner) = scene_with_owner();
    let points = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let handles =
        match scene.add_polyline(owner, &points, PolylineKind::Closed, GuideStyle::default()) {
            Ok(value) => value,
            Err(error) => panic!("closed polyline stores: {error}"),
        };
    assert_eq!(handles.len(), 3);
    assert_eq!(scene.guides().count(), 3);
    assert!(matches!(
        scene.add_polyline(
            owner,
            &[Vec3::ZERO, Vec3::X],
            PolylineKind::Closed,
            GuideStyle::default(),
        ),
        Err(CoreError::InvalidAnnotation { .. })
    ));
}
