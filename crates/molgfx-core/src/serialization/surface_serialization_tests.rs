//! Restored surface controls obey the same field requirements as insertion.
use crate::{
    AtomSelection, RepresentationKind, Scene, SceneDescriptionSources, SurfaceKind, SurfaceStyle,
};

#[test]
fn cold_surface_rehydration_rejects_an_incompatible_field_and_style() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).expect("source");
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selection, RepresentationKind::Surface)
        .expect("surface");
    let representation = scene.representation_mut(handle).expect("surface controls");
    representation.params.surface_kind = SurfaceKind::SolventExcluded;
    representation.params.surface_style = SurfaceStyle::SoftUnion;
    let description = scene.describe();
    assert!(
        Scene::from_description(
            &description,
            SceneDescriptionSources {
                structures: std::slice::from_ref(&structure),
                volumes: &[],
                segmentations: &[],
                atom_properties: &[],
                meshes: &[],
            }
        )
        .is_err()
    );
}

#[test]
fn cold_gaussian_rehydration_preserves_all_authored_sampling_controls() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).expect("source");
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selection, RepresentationKind::Surface)
        .expect("surface");
    let representation = scene.representation_mut(handle).expect("surface controls");
    representation.params.surface_kind = SurfaceKind::Gaussian;
    representation.params.radius_scale = 1.7;
    representation.params.gaussian_sigma = 0.03;
    representation.params.gaussian_cutoff = 6.0;
    representation.params.isolevel = 0.00001;
    representation.params.surface_spacing = Some(0.2);
    let description = scene.describe();
    let rebuilt = Scene::from_description(
        &description,
        SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    )
    .expect("Gaussian surface restore");
    assert_eq!(rebuilt.describe(), description);
}

#[test]
fn analytical_surface_roundtrips_keep_generated_geometry_normals_and_atom_rows() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).expect("source");
    let mut geometry = molframe::surface::IndexedSurfaceMesh::new(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![molframe::surface::SurfaceFace([0, 1, 2])],
    );
    geometry.vertex_normals = vec![[0.0, 0.0, 1.0]; 3];
    let surface = crate::MolecularSurfaceGeometry::new(std::sync::Arc::new(
        molframe::surface::MolecularSurfaceMesh {
            geometry,
            atom_rows: vec![0, 1, 0].into_boxed_slice(),
            discarded_degenerate_faces: 372,
        },
    ))
    .expect("surface");
    let original_identity = surface.cache_identity();
    let selection = scene.add_selection(AtomSelection::All);
    let _ = scene
        .represent(
            selection,
            crate::RepresentationConfig::new(RepresentationKind::MolecularSurface)
                .molecular_surface(surface)
                .surface(crate::SurfaceKind::SolventExcluded, SurfaceStyle::Mesh),
        )
        .expect("representation");
    let description = scene.describe();
    let sources = SceneDescriptionSources {
        structures: std::slice::from_ref(&structure),
        volumes: &[],
        segmentations: &[],
        atom_properties: &[],
        meshes: &[],
    };
    let rebuilt =
        Scene::from_description(&description, sources).expect("analytical surface restore");
    assert_eq!(rebuilt.describe(), description);
    let geometry = rebuilt
        .representations()
        .next()
        .expect("representation")
        .1
        .molecular_surface
        .as_ref()
        .expect("geometry");
    assert_ne!(geometry.cache_identity(), original_identity);
    assert_eq!(&*geometry.mesh().atom_rows, &[0, 1, 0]);
    assert_eq!(geometry.mesh().discarded_degenerate_faces, 372);
    let mut malformed = description;
    malformed.representations[0].molecular_surface = None;
    assert!(Scene::from_description(&malformed, sources).is_err());
}
