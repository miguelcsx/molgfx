use super::tests::{fail, run, scene};
use crate::{Command, ErrorKind, Program, Session, SessionSpec};

#[test]
fn snapshots_restore_symbolic_targets_controls_and_dependencies_through_undo_and_redo() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select core, chain A; select site, $core; show ball_and_stick radius=0.42 as atoms, $site; focus $site; snapshot save initial",
    );
    let initial = session.spec().clone();
    run(
        &mut session,
        &mut scene,
        "select core, chain B; focus chain B",
    );
    let name = crate::Name::new("atoms").expect("layer name");
    session
        .restyle(&mut scene, &name, crate::Form::new(crate::FormKind::Points))
        .expect("form changes");
    let changed = session.spec().clone();
    run(&mut session, &mut scene, "snapshot restore initial");
    assert_eq!(session.spec(), &initial);
    assert_eq!(
        session
            .resolve_selection("$site")
            .expect("restored alias")
            .source(),
        "chain A"
    );
    run(&mut session, &mut scene, "undo");
    assert_eq!(session.spec(), &changed);
    run(&mut session, &mut scene, "redo");
    assert_eq!(session.spec(), &initial);
}

#[test]
fn imported_snapshots_reject_contradictory_forms_and_cyclic_symbolic_targets() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select core, chain A; show points as atoms, $core; snapshot save initial",
    );
    let mut invalid = session.spec().clone();
    let name = crate::Name::new("initial").expect("capture name");
    let atoms = crate::Name::new("atoms").expect("layer name");
    invalid
        .snapshots
        .get_mut(&name)
        .expect("capture")
        .symbols
        .layers
        .get_mut(&atoms)
        .expect("layer")
        .form = crate::Form::new(crate::FormKind::Spacefill);
    assert!(SessionSpec::from_json(&invalid.to_json().expect("encoding")).is_err());
    let mut invalid = session.spec().clone();
    invalid
        .snapshots
        .get_mut(&name)
        .expect("capture")
        .symbols
        .selections
        .insert(
            crate::Name::new("core").expect("alias"),
            crate::QueryText::compile("$core").expect("query"),
        );
    assert!(SessionSpec::from_json(&invalid.to_json().expect("encoding")).is_err());
}

#[test]
fn layers_authored_from_the_previous_selection_remain_capturable_after_selection_is_cleared() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "selection chain A; show points as atoms, $sel; selection; snapshot save initial",
    );
    let saved = authored(&scene);
    let encoded = session.spec().to_json().expect("encoding");
    let mut restored =
        Session::from_spec(SessionSpec::from_json(&encoded).expect("decoding"), &scene);
    run(
        &mut restored,
        &mut scene,
        "hide @atoms; snapshot restore initial",
    );
    assert_eq!(authored(&scene), saved);
    assert!(scene.spec().selected.is_none());
}

fn authored(scene: &molgfx_scene::Scene) -> molgfx_scene::SceneSpec {
    let mut spec = scene.to_spec();
    spec.revision = 0;
    spec
}

#[test]
fn named_snapshots_restore_overlays_and_styles_and_undo_returns_to_the_previous_scene() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "show spacefill as atoms; label \"saved\", chain A; color red, chain A; snapshot save initial",
    );
    let initial = authored(&scene);
    run(
        &mut session,
        &mut scene,
        "hide @atoms; color blue, chain A; label \"later\", chain B",
    );
    let changed = authored(&scene);
    let revision = scene.revision();
    run(&mut session, &mut scene, "snapshot restore initial");
    assert_eq!(authored(&scene), initial);
    assert_eq!(scene.revision(), revision + 1);
    run(&mut session, &mut scene, "undo");
    assert_eq!(authored(&scene), changed);
    run(&mut session, &mut scene, "redo");
    assert_eq!(authored(&scene), initial);
}

#[test]
fn snapshot_names_are_portable_and_save_remove_and_replacement_are_undoable() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let revision = scene.revision();
    run(&mut session, &mut scene, "snapshot save initial");
    assert_eq!(scene.revision(), revision);
    let first = session.spec().snapshots.clone();
    let encoded = session.spec().to_json().expect("session encoding");
    let decoded = SessionSpec::from_json(&encoded).expect("session decoding");
    assert_eq!(decoded.snapshots, first);
    run(
        &mut session,
        &mut scene,
        "show spacefill as atoms; snapshot save initial",
    );
    assert_ne!(session.spec().snapshots, first);
    run(&mut session, &mut scene, "undo");
    assert_eq!(session.spec().snapshots, first);
    run(&mut session, &mut scene, "snapshot remove initial");
    assert!(session.spec().snapshots.is_empty());
    run(&mut session, &mut scene, "undo");
    assert_eq!(session.spec().snapshots, first);
    let mut reloaded = Session::from_spec(decoded, &scene);
    run(&mut reloaded, &mut scene, "snapshot restore initial");
}

#[test]
fn unknown_snapshot_and_later_program_errors_change_neither_scene_nor_names() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let before = scene.to_spec();
    let names = session.spec().clone();
    assert_eq!(
        fail(&mut session, &mut scene, "snapshot restore absent").kind,
        ErrorKind::UnknownSymbol
    );
    assert_eq!(
        fail(
            &mut session,
            &mut scene,
            "snapshot save first; snapshot remove absent"
        )
        .kind,
        ErrorKind::UnknownSymbol
    );
    assert_eq!(scene.spec(), &before);
    assert_eq!(session.spec(), &names);
}

#[test]
fn snapshot_commands_round_trip_and_completion_offers_actions_and_saved_names() {
    for text in [
        "snapshot save initial",
        "snapshot restore initial",
        "snapshot remove initial",
    ] {
        let program = Program::parse(text).expect("command parses");
        let command = &program.statements()[0].command;
        let encoded = serde_json::to_string(command).expect("command encodes");
        let decoded: Command = serde_json::from_str(&encoded).expect("command decodes");
        assert_eq!(&decoded, command);
        assert_eq!(command.to_string(), text);
    }
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "snapshot save initial");
    assert!(
        session
            .completions("snapshot r", 10)
            .iter()
            .any(|entry| entry.text == "restore")
    );
    assert!(
        session
            .completions("snapshot restore i", 18)
            .iter()
            .any(|entry| entry.text == "initial")
    );
}
