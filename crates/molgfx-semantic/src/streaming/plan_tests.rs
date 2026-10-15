use super::*;
use crate::streaming::test_support::structure;
use molgfx_core::Scene;
use molgfx_math::{Camera, Projection, Vec3};

#[test]
fn lod_selection_reuses_frame_capacity_after_warmup() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("LOD selection fixture enters a scene: {error}"),
    };
    if let Err(error) = scene.add_structure(&source) {
        panic!("second LOD selection fixture enters a scene: {error}");
    }
    let index = LodIndex::from_scene(&scene);
    assert!(
        index
            .clusters()
            .iter()
            .all(|cluster| index.cluster(cluster.key) == Some(cluster))
    );
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 10_000.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far: 20_000.0,
        },
    };
    let mut frame = LodFrame::default();
    index.select_into(&camera, [800, 800], LodPolicy::default(), None, &mut frame);
    let capacities = (
        frame.visible.capacity(),
        frame.atom_structures.capacity(),
        frame.levels.capacity(),
    );
    index.select_into(&camera, [800, 800], LodPolicy::default(), None, &mut frame);
    assert_eq!(
        capacities,
        (
            frame.visible.capacity(),
            frame.atom_structures.capacity(),
            frame.levels.capacity(),
        )
    );
    assert_eq!(frame.visible().len(), 2);
}

#[test]
fn hierarchy_selection_uses_residue_and_secondary_error_not_domain_size() {
    let policy = LodPolicy {
        hysteresis: 0.0,
        ..LodPolicy::default()
    };
    assert_eq!(
        choose_hierarchy_level(0.5, 8.0, policy, LodLevel::Atom),
        LodLevel::SecondaryStructure
    );
    assert_eq!(
        choose_hierarchy_level(5.0, 8.0, policy, LodLevel::Domain),
        LodLevel::Atom
    );
    assert_eq!(
        choose_hierarchy_level(0.1, 0.1, policy, LodLevel::Residue),
        LodLevel::Domain
    );
}

#[test]
fn hierarchy_selection_keeps_a_dead_band_at_each_boundary() {
    let policy = LodPolicy::default();
    assert_eq!(
        choose_hierarchy_level(3.9, 8.0, policy, LodLevel::Atom),
        LodLevel::Atom
    );
    assert_eq!(
        choose_hierarchy_level(4.1, 8.0, policy, LodLevel::Residue),
        LodLevel::Residue
    );
}
