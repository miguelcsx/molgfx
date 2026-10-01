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
    assert_eq!(
        region.corners(),
        [
            Vec3::new(-2.0, -1.0, 0.0),
            Vec3::new(2.0, -1.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(-2.0, 1.0, 0.0),
        ]
    );
    let edges = region.edges();
    for index in 0..4 {
        assert_eq!(edges[index].1, edges[(index + 1) % 4].0);
    }
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

#[test]
fn overflowing_planar_vectors_cannot_produce_a_nonfinite_frame() {
    let scene = Scene::from_structure(&fixture::structure()).unwrap();
    let owner = scene.structures().next().unwrap().0;
    for (normal, tangent) in [
        (Vec3::splat(f32::MAX), Vec3::X),
        (Vec3::Z, Vec3::splat(f32::MAX)),
    ] {
        assert!(matches!(
            PlanarRegion::new(owner, Vec3::ZERO, normal, tangent, [4.0, 2.0]),
            Err(CoreError::InvalidPrimitive { .. })
        ));
    }
}

#[test]
fn a_late_invalid_plane_edge_leaves_existing_guides_and_revision_unchanged() {
    let mut scene = Scene::from_structure(&fixture::structure()).unwrap();
    let owner = scene.structures().next().unwrap().0;
    let mut region = PlanarRegion::new(owner, Vec3::ZERO, Vec3::Z, Vec3::X, [4.0, 2.0]).unwrap();
    let original = scene
        .add_guide(crate::Guide::new(owner, Vec3::ZERO, Vec3::Y, GuideStyle::default()).unwrap())
        .unwrap();
    let before: Vec<_> = scene
        .guides()
        .map(|(handle, guide)| (handle, *guide))
        .collect();
    let revision = scene.guide_revision();
    region.bitangent = Vec3::ZERO;
    assert!(matches!(
        scene.add_planar_region(region, GuideStyle::default()),
        Err(CoreError::InvalidAnnotation { .. })
    ));
    assert_eq!(scene.guide_revision(), revision);
    assert_eq!(
        scene
            .guides()
            .map(|(handle, guide)| (handle, *guide))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(scene.guide(original), Some(&before[0].1));
}
