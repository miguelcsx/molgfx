use super::*;

fn owner() -> StructureHandle {
    let mut scene = crate::Scene::new();
    scene
        .add_structure(&crate::fixture::structure())
        .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn bounded_sphere_quadric_lowers_to_finite_triangles() {
    let quadric = Quadric::new(
        [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0],
        Aabb::new(Vec3::splat(-1.5), Vec3::splat(1.5)),
        [8, 8, 8],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mesh = quadric
        .to_mesh(owner(), Rgba8::WHITE, Material::default())
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!mesh.vertices().is_empty());
    assert_eq!(mesh.indices().len() % 3, 0);
    assert!(
        mesh.vertices()
            .iter()
            .all(|vertex| vertex.position.is_finite())
    );
}

#[test]
fn unbounded_sampling_contracts_are_rejected() {
    assert!(
        Quadric::new(
            [0.0; 10],
            Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0)),
            [33, 1, 1]
        )
        .is_err()
    );
}
