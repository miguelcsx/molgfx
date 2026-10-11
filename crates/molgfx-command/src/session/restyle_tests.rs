use super::tests::{run, scene};
use crate::{Form, FormKind, Name, Session};

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
