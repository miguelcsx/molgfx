use super::*;

fn owner() -> StructureHandle {
    let mut scene = crate::Scene::new();
    let structure = crate::fixture::structure();
    match scene.add_structure(&structure) {
        Ok(handle) => handle,
        Err(error) => panic!("fixture places: {error:?}"),
    }
}

fn triangle() -> (Vec<MeshVertex>, Vec<u32>) {
    let vertex = |x: f32, y: f32| MeshVertex {
        position: Vec3::new(x, y, 0.0),
        normal: Vec3::Z,
        color: Rgba8::opaque(200, 120, 60),
    };
    (
        vec![vertex(0.0, 0.0), vertex(1.0, 0.0), vertex(0.0, 1.0)],
        vec![0, 1, 2],
    )
}

#[test]
fn a_valid_triangle_is_stored_with_normalized_normals() {
    let (mut vertices, indices) = triangle();
    // An unnormalized normal must not reach the lighting as-is.
    vertices[0].normal = Vec3::new(0.0, 0.0, 7.5);
    let Ok(mesh) = Mesh::new(owner(), vertices, indices, Material::default()) else {
        panic!("a well-formed triangle is accepted")
    };
    assert_eq!(mesh.indices().len(), 3);
    let Some(first) = mesh.vertices().first() else {
        panic!("the mesh keeps its vertices")
    };
    assert!((first.normal.length() - 1.0).abs() < 1.0e-5);
    assert!(mesh.visible());
}

#[test]
fn an_index_outside_the_vertex_range_is_rejected() {
    let (vertices, _) = triangle();
    assert!(Mesh::new(owner(), vertices, vec![0, 1, 9], Material::default()).is_err());
}

#[test]
fn an_index_count_that_is_not_triangles_is_rejected() {
    let (vertices, _) = triangle();
    assert!(Mesh::new(owner(), vertices, vec![0, 1], Material::default()).is_err());
}

#[test]
fn empty_geometry_is_rejected() {
    assert!(Mesh::new(owner(), Vec::new(), Vec::new(), Material::default()).is_err());
    let (vertices, _) = triangle();
    assert!(Mesh::new(owner(), vertices, Vec::new(), Material::default()).is_err());
}

#[test]
fn a_non_finite_position_is_rejected() {
    let (mut vertices, indices) = triangle();
    vertices[1].position = Vec3::new(f32::NAN, 0.0, 0.0);
    assert!(Mesh::new(owner(), vertices, indices, Material::default()).is_err());
}

#[test]
fn bounds_enclose_every_vertex() {
    let (vertices, indices) = triangle();
    let Ok(mesh) = Mesh::new(owner(), vertices, indices, Material::default()) else {
        panic!("a well-formed triangle is accepted")
    };
    let bounds = mesh.bounds();
    for vertex in mesh.vertices() {
        let point = vertex.position;
        assert!(
            point.x >= bounds.min.x
                && point.y >= bounds.min.y
                && point.z >= bounds.min.z
                && point.x <= bounds.max.x
                && point.y <= bounds.max.y
                && point.z <= bounds.max.z,
            "bounds enclose the mesh"
        );
    }
}

#[test]
fn provider_surfaces_keep_normals_color_and_component_policy() {
    use pdbiox::surface::{IndexedSurfaceMesh, SurfaceFace};

    let surface = IndexedSurfaceMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.1, 0.0, 0.0],
            [10.0, 0.1, 0.0],
        ],
        vec![SurfaceFace([0, 1, 2]), SurfaceFace([3, 4, 5])],
    );
    let Ok(policy) = SurfaceComponentPolicy::minimum_area(0.1)
        .and_then(|policy| policy.with_maximum_components(1))
    else {
        panic!("component policy validates")
    };
    let color = Rgba8::opaque(25, 100, 220);
    let Ok(mesh) = Mesh::from_surface(owner(), &surface, color, Material::default(), policy) else {
        panic!("provider surface converts")
    };

    assert_eq!(mesh.vertices().len(), 3);
    assert_eq!(mesh.indices(), &[0, 1, 2]);
    assert_eq!(mesh.component_policy(), policy);
    assert!(mesh.vertices().iter().all(|vertex| vertex.color == color));
    assert!(
        mesh.vertices()
            .iter()
            .all(|vertex| (vertex.normal.length() - 1.0).abs() < 1.0e-5)
    );
}

#[test]
fn triangle_stream_topologies_lower_to_stable_indices() {
    let vertex = |x: f32, y: f32| MeshVertex {
        position: Vec3::new(x, y, 0.0),
        normal: Vec3::Z,
        color: Rgba8::WHITE,
    };
    let quad = vec![
        vertex(0.0, 0.0),
        vertex(1.0, 0.0),
        vertex(1.0, 1.0),
        vertex(0.0, 1.0),
    ];
    let Ok(strip) = Mesh::from_topology(
        owner(),
        quad.clone(),
        MeshTopology::TriangleStrip,
        Material::default(),
    ) else {
        panic!("triangle strip validates")
    };
    let Ok(fan) = Mesh::from_topology(
        owner(),
        quad.clone(),
        MeshTopology::TriangleFan,
        Material::default(),
    ) else {
        panic!("triangle fan validates")
    };
    let Ok(quads) = Mesh::from_topology(owner(), quad, MeshTopology::Quads, Material::default())
    else {
        panic!("quad stream validates")
    };
    assert_eq!(strip.indices(), &[0, 1, 2, 2, 1, 3]);
    assert_eq!(fan.indices(), &[0, 1, 2, 0, 2, 3]);
    assert_eq!(quads.indices(), &[0, 1, 2, 0, 2, 3]);
}

#[test]
fn malformed_topology_streams_are_rejected() {
    let (vertices, _) = triangle();
    assert!(
        Mesh::from_topology(
            owner(),
            vertices[..2].to_vec(),
            MeshTopology::TriangleStrip,
            Material::default(),
        )
        .is_err()
    );
    assert!(
        Mesh::from_topology(owner(), vertices, MeshTopology::Quads, Material::default(),).is_err()
    );
}
