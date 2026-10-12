use super::tests::{run, scene};
use crate::{Form, FormKind, Name, Session};

#[test]
fn a_layer_form_can_be_reviewed_on_a_worker_without_mutating_its_scene() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "show spacefill as metal, element Fe",
    );
    let before = scene.to_spec();
    let name = Name::new("metal").expect("layer name");
    let form = session
        .layer_form_specification(&scene, &name, Form::new(FormKind::Lines))
        .expect("immutable form");
    let source = scene.sources().values().next().expect("source").clone();
    let review = std::thread::spawn(move || form.applicability(&source).expect("geometry review"))
        .join()
        .expect("worker");
    assert!(
        review.is_applicable(),
        "Lines marks selected isolated atoms"
    );
    assert_eq!(review.selected_atoms, 1);
    assert_eq!(scene.spec(), &before);
}

#[test]
fn a_restyle_keeps_layer_identity_and_can_be_undone_and_redone() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "show spacefill as metal, element Fe",
    );
    run(&mut session, &mut scene, "hide @metal");
    let name = Name::new("metal").unwrap();
    let id = session.spec().layers[&name].id;
    let before = scene.spec().representations[&id].clone();
    session
        .restyle(&mut scene, &name, Form::new(FormKind::Points))
        .unwrap();
    assert_eq!(session.spec().layers[&name].id, id);
    assert_eq!(
        scene.spec().representations[&id].opacity().to_bits(),
        before.opacity().to_bits()
    );
    assert!(!scene.spec().representations[&id].is_visible());
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().representations[&id], before);
    assert_eq!(
        session.spec().layers[&name].form.kind(),
        FormKind::Spacefill
    );
    run(&mut session, &mut scene, "redo");
    assert_eq!(session.spec().layers[&name].id, id);
    assert_eq!(scene.spec().representations[&id].form_name(), "points");
}

#[test]
fn a_failed_restyle_preserves_the_scene_session_and_previous_history() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "show spacefill as metal, element Fe",
    );
    let before = scene.to_spec();
    let names = session.spec().clone();
    let name = Name::new("metal").unwrap();
    assert!(
        session
            .restyle(&mut scene, &name, Form::new(FormKind::Cartoon))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
    assert_eq!(session.spec(), &names);
    run(&mut session, &mut scene, "undo");
    assert!(!session.spec().layers.contains_key(&name));
}

#[test]
fn an_analytical_restyle_binds_geometry_atomically_and_keeps_undo_and_redo_complete() {
    use molgfx_scene::{
        DataSource, MolecularSurfaceBinding, MolecularSurfaceControls, MolecularSurfaceMethod,
    };
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "show spacefill as metal, element Fe",
    );
    run(&mut session, &mut scene, "hide @metal");
    let name = Name::new("metal").expect("layer");
    let id = session.spec().layers[&name].id;
    let previous = scene.spec().representations[&id].clone();
    let hash = "a".repeat(64);
    let mut form = Form::new(FormKind::Surf);
    form.set_option("source", &hash).expect("source");
    let (&structure, molecule) = scene.sources().first_key_value().expect("molecule");
    let molecule = molecule.clone();
    let atom = u32::try_from(
        molecule
            .topology()
            .atoms
            .iter()
            .position(|atom| atom.element == 26)
            .expect("iron"),
    )
    .expect("row");
    let make_binding = |method| {
        MolecularSurfaceBinding::new(
            DataSource::new(hash.clone()),
            structure,
            &molecule,
            method,
            MolecularSurfaceControls::default(),
            molgfx_scene::source::MolecularSurfaceDescription {
                discarded_degenerate_faces: 0,
                vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                faces: vec![[0, 1, 2]],
                atom_rows: vec![atom; 3],
            }
            .into_geometry()
            .expect("asset"),
        )
        .expect("binding")
    };
    let before = scene.to_spec();
    assert!(
        session
            .restyle_with_molecular_surface(
                &mut scene,
                &name,
                form,
                make_binding(MolecularSurfaceMethod::Msms)
            )
            .is_err()
    );
    assert_eq!(scene.to_spec(), before);
    session
        .restyle_with_molecular_surface(
            &mut scene,
            &name,
            form,
            make_binding(MolecularSurfaceMethod::Surf),
        )
        .expect("atomic restyle");
    assert_eq!(scene.revision(), before.revision + 1);
    assert!(!scene.spec().representations[&id].is_visible());
    assert!(scene.unresolved_representations().is_empty());
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().representations[&id], previous);
    run(&mut session, &mut scene, "redo");
    assert_eq!(scene.spec().representations[&id].form_name(), "surf");
    assert!(scene.unresolved_representations().is_empty());
}
