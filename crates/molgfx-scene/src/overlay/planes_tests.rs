use super::PlaneSpec;
use crate::appearance::tests::two_chains;
use crate::{Color, Error, PatchOperation, PlaneId, Scene, ScenePatch, StructureId};
use molgfx_math::Vec3;

fn scene() -> Scene {
    Scene::from_structure(&two_chains()).unwrap()
}

fn plane() -> PlaneSpec {
    let mut plane = PlaneSpec::new(
        StructureId::new(1),
        [3.0, -4.0, 5.0],
        [0.0, 0.0, 2.0],
        [1.0, 0.0, 2.0],
        [4.0, 2.0],
    );
    plane.color = Color::rgb(21, 83, 147);
    plane.width_pixels = 3.25;
    plane.opacity = 0.375;
    plane
}

fn geometry(scene: &Scene) -> Vec<(Vec3, Vec3, molgfx_core::GuideStyle)> {
    scene
        .resolved()
        .guides()
        .map(|(_, guide)| (guide.start(), guide.end(), guide.style()))
        .collect()
}

#[test]
fn planar_spec_and_deserialized_validation_agree_with_core_geometry() {
    let scene = scene();
    let base = plane();
    let mut cases = vec![base];
    for (normal, tangent) in [
        ([0.0; 3], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, 0.0005], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, 2.0], [0.0, 0.0, 7.0]),
        ([0.0, 0.0, 2.0], [0.0005, 0.0, 7.0]),
        ([f32::MAX; 3], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, 2.0], [f32::MAX; 3]),
        ([0.0, 0.0, 2.0], [1.0, 0.0, 10000.0]),
    ] {
        cases.push(PlaneSpec {
            normal,
            tangent,
            ..base
        });
    }
    cases.push(PlaneSpec {
        size: [1.0e-8, 2.0],
        ..base
    });
    cases.push(PlaneSpec {
        center: [f32::MAX; 3],
        ..base
    });
    for candidate in cases {
        let core = molgfx_core::PlanarRegion::validate_geometry(
            Vec3::from_array(candidate.center),
            Vec3::from_array(candidate.normal),
            Vec3::from_array(candidate.tangent),
            candidate.size,
        );
        let restored: PlaneSpec =
            serde_json::from_str(&serde_json::to_string(&candidate).unwrap()).unwrap();
        assert_eq!(candidate.validate(scene.spec()).is_ok(), core.is_ok());
        assert_eq!(restored.validate(scene.spec()).is_ok(), core.is_ok());
    }
    for candidate in [
        PlaneSpec {
            center: [f32::NAN; 3],
            ..base
        },
        PlaneSpec {
            normal: [f32::INFINITY; 3],
            ..base
        },
    ] {
        assert!(matches!(
            candidate.validate(scene.spec()),
            Err(Error::InvalidSpec(_))
        ));
    }
}

#[test]
fn a_skew_plane_remove_and_inverse_restore_exact_geometry_and_style() {
    let mut scene = scene();
    let expected = plane();
    let id = scene.add(expected).unwrap();
    let original = scene.to_spec();
    let before = geometry(&scene);
    let corners = [
        Vec3::new(1.0, -5.0, 5.0),
        Vec3::new(5.0, -5.0, 5.0),
        Vec3::new(5.0, -3.0, 5.0),
        Vec3::new(1.0, -3.0, 5.0),
    ];
    assert_eq!(
        before
            .iter()
            .map(|(start, end, _)| (*start, *end))
            .collect::<Vec<_>>(),
        (0..4)
            .map(|index| (corners[index], corners[(index + 1) % 4]))
            .collect::<Vec<_>>()
    );
    for (_, _, style) in &before {
        assert_eq!(style.color, expected.color.native());
        assert_eq!(
            style.width_pixels.to_bits(),
            expected.width_pixels.to_bits()
        );
        assert_eq!(style.opacity.to_bits(), expected.opacity.to_bits());
    }
    let remove = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::RemovePlane { id }],
    };
    let undo = remove.inverse(&original).unwrap();
    scene.apply(&remove).unwrap();
    assert!(!scene.spec().planes.contains_key(&id));
    assert!(geometry(&scene).is_empty());
    scene.apply(&undo).unwrap();
    assert_eq!(scene.spec().planes.get(&id), Some(&expected));
    assert_eq!(geometry(&scene), before);
}

#[test]
fn an_invalid_plane_after_removal_does_not_commit_partial_semantic_or_native_state() {
    for invalid in [
        PlaneSpec {
            tangent: [0.0, 0.0, 2.0],
            ..plane()
        },
        PlaneSpec {
            structure: StructureId::new(99),
            ..plane()
        },
    ] {
        let mut scene = scene();
        let id = scene.add(plane()).unwrap();
        let before = scene.to_spec();
        let guides = geometry(&scene);
        let guide_revision = scene.resolved().guide_revision();
        let patch = ScenePatch {
            base_revision: scene.revision(),
            operations: vec![
                PatchOperation::RemovePlane { id },
                PatchOperation::AddPlane {
                    id: PlaneId::new(99),
                    spec: invalid,
                },
            ],
        };
        assert!(matches!(scene.apply(&patch), Err(Error::InvalidSpec(_))));
        assert_eq!(scene.spec(), &before);
        assert_eq!(geometry(&scene), guides);
        assert_eq!(scene.resolved().guide_revision(), guide_revision);
    }
}
