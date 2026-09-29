use super::tests::structure;
use crate::Session;
use molgfx_scene::Scene;

fn scene() -> Scene {
    match Scene::from_structure(&structure()) {
        Ok(scene) => scene,
        Err(error) => panic!("scene builds: {error}"),
    }
}

#[test]
fn a_label_command_adds_one_annotation_and_undo_removes_it() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    session
        .execute_text(&mut scene, "label \"Haem, iron\", resname HEM")
        .unwrap_or_else(|errors| panic!("{errors}"));
    assert_eq!(scene.spec().annotations.len(), 1);
    let Some(annotation) = scene.spec().annotations.values().next() else {
        panic!("one annotation")
    };
    assert_eq!(annotation.text.as_ref(), "Haem, iron");
    session
        .execute_text(&mut scene, "undo")
        .unwrap_or_else(|errors| panic!("{errors}"));
    assert!(scene.spec().annotations.is_empty());
}

#[test]
fn distance_angle_and_dihedral_commands_take_exactly_their_arity() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    for text in [
        "distance in two, resname HEM, chain A",
        "distance, resname HEM, chain A",
        "angle, resname HEM, chain A, chain B",
        "dihedral, resname HEM, chain A, chain B, name CA",
    ] {
        // A single structure needs no `in`, so the first form is refused for
        // naming a structure that does not exist.
        let result = session.execute_text(&mut scene, text);
        assert_eq!(result.is_ok(), !text.contains(" in two"), "{text}");
    }
    assert_eq!(scene.spec().measurements.len(), 3);
    assert!(
        session
            .execute_text(&mut scene, "distance, chain A")
            .is_err()
    );
    assert!(
        session
            .execute_text(&mut scene, "angle, a, b, c, d")
            .is_err()
    );
}

#[test]
fn label_and_measurement_commands_print_text_that_parses_back() {
    for text in [
        "label \"a \\\"quoted\\\" word\", chain A",
        "distance, chain A, chain B",
        "dihedral, chain A, chain B, name CA, resname HEM",
    ] {
        let program = crate::Program::parse(text).unwrap_or_else(|e| panic!("{}", e.render(text)));
        let printed = program
            .statements()
            .iter()
            .map(|statement| statement.command.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        let reparsed =
            crate::Program::parse(&printed).unwrap_or_else(|e| panic!("{}", e.render(&printed)));
        assert_eq!(
            program.statements()[0].command,
            reparsed.statements()[0].command,
            "{text}"
        );
    }
}
