use super::*;
use crate::fixture;
use crate::{AnnotationAnchor, GuideStyle, MarkerStyle, Scene, ValidationKind, ValidationMarker};
use molgfx_math::{Mat4, Quat, Rgba8, Vec3};

#[test]
fn a_triclinic_cell_produces_finite_corners_and_edges() {
    let cell = match CrystalCell::new([10.0, 11.0, 12.0], [80.0, 90.0, 100.0]) {
        Ok(cell) => cell,
        Err(error) => panic!("cell validates: {error}"),
    };
    assert_eq!(cell.corners().len(), 8);
    assert!(cell.corners().iter().all(|corner| corner.is_finite()));
    assert_eq!(cell.edges().len(), 12);
    assert!(cell.edges().iter().all(|(start, end)| start != end));
}

#[test]
fn a_singular_cell_and_tensor_are_rejected() {
    assert!(CrystalCell::new([1.0, 1.0, 1.0], [90.0, 90.0, 0.0]).is_err());
    assert!(AnisotropicEllipsoid::new(Vec3::ZERO, [1.0, 1.0, 1.0, 2.0, 0.0, 0.0]).is_err());
}

#[test]
fn an_anisotropic_ellipsoid_has_a_stable_ray_interval() {
    let ellipsoid = match AnisotropicEllipsoid::new(Vec3::ZERO, [4.0, 1.0, 1.0, 0.0, 0.0, 0.0]) {
        Ok(ellipsoid) => ellipsoid,
        Err(error) => panic!("tensor validates: {error}"),
    };
    assert_eq!(
        ellipsoid.ray_intersection(Vec3::new(-3.0, 0.0, 0.0), Vec3::X),
        Some((1.0, 5.0))
    );
    assert_eq!(ellipsoid.bounds().min, Vec3::new(-2.0, -1.0, -1.0));
}

#[test]
fn a_symbol_and_validation_marker_reject_non_finite_input() {
    let source = fixture::structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let anchor = match AnnotationAnchor::world(Vec3::ZERO) {
        Ok(anchor) => anchor,
        Err(error) => panic!("anchor validates: {error}"),
    };
    assert!(
        ValidationMarker::new(
            owner,
            anchor,
            ValidationKind::Clash,
            1.5,
            MarkerStyle::default(),
        )
        .is_err()
    );
    let marker = match ValidationMarker::new(
        owner,
        anchor,
        ValidationKind::Geometry,
        0.75,
        MarkerStyle::default(),
    ) {
        Ok(marker) => marker,
        Err(error) => panic!("validation marker validates: {error}"),
    };
    assert!(scene.add_validation_marker(marker).is_ok());
    assert!(
        CarbohydrateSymbol::new(
            owner,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::splat(-1.0),
            CarbohydrateShape::Glc,
            Rgba8::WHITE,
        )
        .is_err()
    );
    let instance = match SymmetryInstance::new(0, Mat4::IDENTITY) {
        Ok(instance) => instance,
        Err(error) => panic!("identity instance validates: {error}"),
    };
    assert_eq!(instance.id, 0);
}

#[test]
fn a_unit_cell_lowers_to_twelve_pickable_guides() {
    let source = fixture::structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let cell = match CrystalCell::new([4.0, 5.0, 6.0], [90.0, 90.0, 90.0]) {
        Ok(cell) => cell,
        Err(error) => panic!("cell validates: {error}"),
    };
    let handles = match scene.add_unit_cell(owner, cell, GuideStyle::default()) {
        Ok(handles) => handles,
        Err(error) => panic!("unit cell lowers: {error}"),
    };
    assert_eq!(handles.len(), 12);
    assert_eq!(scene.guides().count(), 12);
    assert!(scene.world_aabb().max.z >= 6.0);
}
