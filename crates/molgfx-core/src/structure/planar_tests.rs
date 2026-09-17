use super::*;
use crate::{GuideStyle, Scene, fixture};
use molgfx_math::Vec3;

#[test]
fn a_planar_region_orthogonalizes_its_tangent_and_closes_its_boundary() {
    let mut scene = match Scene::from_structure(&fixture::structure()) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let region = match PlanarRegion::new(
        owner,
        Vec3::ZERO,
        Vec3::Z,
        Vec3::new(1.0, 0.0, 2.0),
        [4.0, 2.0],
    ) {
        Ok(region) => region,
        Err(error) => panic!("region validates: {error}"),
    };
    assert!(region.normal.dot(region.tangent).abs() < 1.0e-6);
    assert_eq!(region.edges().len(), 4);
    let guide = match crate::Guide::new(
        owner,
        region.edges()[0].0,
        region.edges()[0].1,
        GuideStyle::default(),
    ) {
        Ok(guide) => guide,
        Err(error) => panic!("guide validates: {error}"),
    };
    assert!(scene.add_guide(guide).is_ok());
}

#[test]
fn a_degenerate_planar_region_is_rejected() {
    let scene = match Scene::from_structure(&fixture::structure()) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    assert!(PlanarRegion::new(owner, Vec3::ZERO, Vec3::Z, Vec3::Z, [1.0, 1.0]).is_err());
}
