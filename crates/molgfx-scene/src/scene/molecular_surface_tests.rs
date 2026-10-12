//! Analytical source bindings resolve atomically and retain scientific controls.
use crate::{
    DataSource, MolecularSurfaceBinding, MolecularSurfaceControls, MolecularSurfaceGeometry,
    MolecularSurfaceMethod, Scene, rep,
};
use std::sync::Arc;

fn oxygen() -> molframe::Structure {
    molframe::read_bytes(
        b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n"
            .to_vec(),
        Some("oxygen.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0
}

fn geometry() -> MolecularSurfaceGeometry {
    let mut geometry = molframe::surface::IndexedSurfaceMesh::new(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![molframe::surface::SurfaceFace([0, 1, 2])],
    );
    geometry.vertex_normals = vec![[0.0, 0.0, 1.0]; 3];
    MolecularSurfaceGeometry::new(Arc::new(molframe::surface::MolecularSurfaceMesh {
        geometry,
        atom_rows: vec![0; 3].into_boxed_slice(),
        discarded_degenerate_faces: 0,
    }))
    .expect("surface")
}

#[test]
fn sampled_analytical_bindings_refuse_stale_worker_results_without_consuming_their_identity() {
    let mut scene = Scene::from_structure(&oxygen()).expect("scene");
    let structure = crate::StructureId::new(1);
    scene
        .set_trajectory_pair(
            structure,
            crate::TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3]])),
            crate::TrajectoryFrame::new(1, 1.0, Arc::from([[2.0, 0.0, 0.0]])),
            0.25,
        )
        .expect("pair");
    let pose = scene.molecular_pose(structure).expect("capture");
    let source = DataSource::new("sampled-provider-result");
    let id = scene
        .add(rep::surf("all", source.clone()))
        .expect("pending");
    assert!(
        !scene
            .molecular_surface_is_current(id)
            .expect("pending status")
    );
    let binding = MolecularSurfaceBinding::new(
        source,
        structure,
        pose.source(),
        MolecularSurfaceMethod::Surf,
        MolecularSurfaceControls::default(),
        geometry(),
    )
    .expect("binding")
    .at_pose(&pose)
    .expect("sample");
    scene.set_trajectory_time(structure, 0.5).expect("advance");
    let before = scene.to_spec();
    assert!(scene.bind_molecular_surface(binding.clone()).is_err());
    assert_eq!(scene.to_spec(), before);
    assert_eq!(scene.unresolved_representations(), vec![id]);
    scene.set_trajectory_time(structure, 0.25).expect("reverse");
    scene
        .bind_molecular_surface(binding)
        .expect("same exact sample");
    assert!(
        scene
            .molecular_surface_is_current(id)
            .expect("current status")
    );
    scene
        .set_opacity(id, 0.4)
        .expect("appearance edit preserves sample");
    let native = scene
        .resolved
        .representation(scene.representation_handle(id).expect("handle"))
        .expect("native");
    let placed = scene.resolved.structures().next().expect("placement").1;
    native
        .molecular_surface
        .as_ref()
        .expect("asset")
        .validate_pose(placed)
        .expect("retained sample");
    scene
        .set_trajectory_time(structure, 0.5)
        .expect("advance after binding");
    assert!(
        !scene
            .molecular_surface_is_current(id)
            .expect("stale status")
    );
    let placed = scene.resolved.structures().next().expect("placement").1;
    assert!(
        scene
            .resolved
            .representation(scene.representation_handle(id).expect("handle"))
            .expect("native")
            .molecular_surface
            .as_ref()
            .expect("asset")
            .validate_pose(placed)
            .is_err()
    );
}

#[test]
fn unresolved_analytical_surfaces_keep_portable_descriptors_until_their_actual_geometry_is_bound() {
    let molecule = oxygen();
    let mut scene = Scene::from_structure(&molecule).expect("scene");
    let source = DataSource::new("varshney-result");
    let id = scene
        .add(rep::surf("all", source.clone()).wire(true))
        .expect("descriptor");
    assert_eq!(scene.unresolved_representations(), vec![id]);
    assert!(scene.representation_handle(id).is_none());
    let mut descriptor = scene.to_spec();
    let (&structure, molecular_source) = scene.sources().first_key_value().expect("source");
    let binding = MolecularSurfaceBinding::new(
        source,
        structure,
        molecular_source,
        MolecularSurfaceMethod::Surf,
        MolecularSurfaceControls::default(),
        geometry(),
    )
    .expect("binding");
    scene.bind_molecular_surface(binding).expect("bind");
    assert!(scene.unresolved_representations().is_empty());
    assert_eq!(scene.revision(), descriptor.revision + 1);
    descriptor.revision += 1;
    assert_eq!(scene.to_spec(), descriptor);
    let handle = scene.representation_handle(id).expect("resolved handle");
    let native = scene.resolved.representation(handle).expect("native");
    assert_eq!(
        native.kind,
        molgfx_core::RepresentationKind::MolecularSurface
    );
    assert_eq!(native.params.surface_style, molgfx_core::SurfaceStyle::Mesh);
    let original = native
        .molecular_surface
        .as_ref()
        .expect("asset")
        .mesh()
        .geometry
        .vertices
        .as_ptr();
    scene.set_opacity(id, 0.4).expect("appearance edit");
    let handle = scene.representation_handle(id).expect("handle");
    assert_eq!(
        scene
            .resolved
            .representation(handle)
            .expect("native")
            .molecular_surface
            .as_ref()
            .expect("asset")
            .mesh()
            .geometry
            .vertices
            .as_ptr(),
        original
    );
}

#[test]
fn a_wrong_provider_or_probe_radius_is_refused_without_consuming_the_pending_binding() {
    let mut scene = Scene::from_structure(&oxygen()).expect("scene");
    let source = DataSource::new("sanner-result");
    let id = scene
        .add(rep::msms("all", source.clone()))
        .expect("descriptor");
    let (&structure, molecule) = scene.sources().first_key_value().expect("source");
    let molecule = molecule.clone();
    let before = scene.to_spec();
    for (method, probe_radius) in [
        (MolecularSurfaceMethod::Surf, 1.4),
        (MolecularSurfaceMethod::Msms, 2.0),
    ] {
        let controls = MolecularSurfaceControls {
            probe_radius,
            ..MolecularSurfaceControls::default()
        };
        let binding = MolecularSurfaceBinding::new(
            source.clone(),
            structure,
            &molecule,
            method,
            controls,
            geometry(),
        )
        .expect("binding");
        assert!(scene.bind_molecular_surface(binding).is_err());
        assert_eq!(scene.to_spec(), before);
        assert_eq!(scene.unresolved_representations(), vec![id]);
    }
    let binding = MolecularSurfaceBinding::new(
        source,
        structure,
        &molecule,
        MolecularSurfaceMethod::Msms,
        MolecularSurfaceControls::default(),
        geometry(),
    )
    .expect("binding");
    scene
        .bind_molecular_surface(binding)
        .expect("correct retry");
    assert!(scene.unresolved_representations().is_empty());
}

#[test]
fn transported_provider_output_uses_the_native_atomic_validation_and_moves_derived_columns() {
    let mut scene = Scene::from_structure(&oxygen()).expect("scene");
    let id = scene
        .add(rep::surf("all", DataSource::new("transport")))
        .expect("descriptor");
    let mut data = crate::MolecularSurfaceData {
        method: MolecularSurfaceMethod::Surf,
        controls: MolecularSurfaceControls::default(),
        mesh: molgfx_core::MolecularSurfaceDescription::from_geometry(&geometry()),
    };
    let before = scene.to_spec();
    data.mesh.normals[0] = [f32::NAN, 0.0, 1.0];
    assert!(scene.bind_molecular_surface_data(id, data.clone()).is_err());
    assert_eq!(scene.to_spec(), before);
    data.mesh.normals[0] = [0.0, 0.0, 1.0];
    let vertices = data.mesh.vertices.as_ptr();
    scene
        .bind_molecular_surface_data(id, data)
        .expect("correct retry");
    let native = scene
        .resolved
        .representation(scene.representation_handle(id).expect("handle"))
        .expect("native");
    assert_eq!(
        native
            .molecular_surface
            .as_ref()
            .expect("asset")
            .mesh()
            .geometry
            .vertices
            .as_ptr(),
        vertices
    );
}

#[test]
fn identical_analytical_computations_reuse_the_existing_asset_but_changed_geometry_is_refused() {
    let mut scene = Scene::from_structure(&oxygen()).expect("scene");
    let source = DataSource::new("same-computation");
    let id = scene
        .add(rep::surf("all", source.clone()))
        .expect("descriptor");
    let (&structure, molecule) = scene.sources().first_key_value().expect("source");
    let molecule = molecule.clone();
    let binding = || {
        MolecularSurfaceBinding::new(
            source.clone(),
            structure,
            &molecule,
            MolecularSurfaceMethod::Surf,
            MolecularSurfaceControls::default(),
            geometry(),
        )
        .expect("binding")
    };
    scene.bind_molecular_surface(binding()).expect("first");
    let native = scene
        .resolved
        .representation(scene.representation_handle(id).expect("handle"))
        .expect("native");
    let original = native
        .molecular_surface
        .as_ref()
        .expect("asset")
        .cache_identity();
    scene
        .bind_molecular_surface(binding())
        .expect("identical independent result");
    let native = scene
        .resolved
        .representation(scene.representation_handle(id).expect("handle"))
        .expect("native");
    assert_eq!(
        native
            .molecular_surface
            .as_ref()
            .expect("asset")
            .cache_identity(),
        original
    );
    let mut data = crate::MolecularSurfaceData {
        method: MolecularSurfaceMethod::Surf,
        controls: MolecularSurfaceControls::default(),
        mesh: molgfx_core::MolecularSurfaceDescription::from_geometry(&geometry()),
    };
    data.mesh.vertices[0][0] = -0.5;
    let before = scene.to_spec();
    assert!(scene.bind_molecular_surface_data(id, data).is_err());
    assert_eq!(scene.to_spec(), before);
}

#[test]
fn one_surface_content_identity_can_be_bound_to_independent_molecular_instances() {
    let molecule = oxygen();
    let source = crate::source::MolecularSource::from_molframe(&molecule);
    let mut scene = Scene::empty();
    let first = scene.add_source(&source).expect("first instance");
    let second = scene.add_source(&source).expect("second instance");
    let content = DataSource::new("shared-scientific-content");
    let a = scene
        .add(rep::surf("all", content.clone()).structure(first))
        .expect("first recipe");
    let b = scene
        .add(rep::surf("all", content.clone()).structure(second))
        .expect("second recipe");
    let asset = geometry();
    for structure in [first, second] {
        let molecule = scene.sources().get(&structure).expect("instance");
        let binding = MolecularSurfaceBinding::new(
            content.clone(),
            structure,
            molecule,
            MolecularSurfaceMethod::Surf,
            MolecularSurfaceControls::default(),
            asset.clone(),
        )
        .expect("binding");
        scene
            .bind_molecular_surface(binding)
            .expect("independently scoped binding");
    }
    assert!(scene.unresolved_representations().is_empty());
    assert!(scene.representation_handle(a).is_some());
    assert!(scene.representation_handle(b).is_some());
}
