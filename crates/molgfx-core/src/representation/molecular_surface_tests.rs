use super::*;

fn triangle() -> molframe::surface::MolecularSurfaceMesh {
    let mut geometry = molframe::surface::IndexedSurfaceMesh::new(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![molframe::surface::SurfaceFace([0, 1, 2])],
    );
    geometry.vertex_normals = vec![[0.0, 0.0, 1.0]; 3];
    molframe::surface::MolecularSurfaceMesh {
        geometry,
        atom_rows: vec![4, 6, 8].into_boxed_slice(),
        discarded_degenerate_faces: 0,
    }
}

#[test]
fn molecular_surfaces_share_the_provider_and_preserve_each_original_atom_identity() {
    let source = Arc::new(triangle());
    let pointer = source.geometry.vertices.as_ptr();
    let surface = MolecularSurfaceGeometry::new(source.clone()).expect("surface");
    assert_eq!(surface.mesh().geometry.vertices.as_ptr(), pointer);
    assert_eq!(&*surface.mesh().atom_rows, &[4, 6, 8]);
    assert_eq!(Arc::strong_count(&source), 2);
    assert!(surface.validate_atom_count(8).is_err());
    surface.validate_atom_count(9).expect("original rows exist");
    assert_eq!(surface, surface.clone());
}

#[test]
fn malformed_provider_geometry_is_refused_even_when_its_cached_report_claims_validity() {
    let mut mesh = triangle();
    mesh.geometry.faces[0].0 = [0, 0, 2];
    assert!(MolecularSurfaceGeometry::new(Arc::new(mesh)).is_err());
    let mut mesh = triangle();
    mesh.geometry.vertices[0][0] = f32::NAN;
    assert!(MolecularSurfaceGeometry::new(Arc::new(mesh)).is_err());
    let mut mesh = triangle();
    mesh.geometry.vertex_normals[0] = [0.0; 3];
    assert!(MolecularSurfaceGeometry::new(Arc::new(mesh)).is_err());
    let mut mesh = triangle();
    mesh.atom_rows = vec![0].into_boxed_slice();
    assert!(MolecularSurfaceGeometry::new(Arc::new(mesh)).is_err());
}

#[test]
fn independently_owned_surface_geometry_reuses_content_only_when_normals_faces_and_atom_owners_agree()
 {
    let reference = MolecularSurfaceGeometry::new(Arc::new(triangle())).expect("reference");
    let same = MolecularSurfaceGeometry::new(Arc::new(triangle())).expect("independent output");
    assert_ne!(reference, same, "independent ownership remains explicit");
    assert!(reference.same_content(&same));
    let mut normals = triangle();
    normals.geometry.vertex_normals[0] = [0.0, 1.0, 0.0];
    let mut faces = triangle();
    faces.geometry.faces[0].0 = [0, 2, 1];
    let mut atoms = triangle();
    atoms.atom_rows[0] = 2;
    for mesh in [normals, faces, atoms] {
        assert!(!reference.same_content(
            &MolecularSurfaceGeometry::new(Arc::new(mesh)).expect("valid different data")
        ));
    }
}
