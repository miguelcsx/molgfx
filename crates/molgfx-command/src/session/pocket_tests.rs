//! Session coverage for the `pocket` composition.

use super::tests::{fail, operations, run, scene};
use crate::{Command, ErrorKind, Session};

#[test]
fn pocket_adds_six_layers_around_the_subject_and_focuses_it() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let outcome = run(&mut session, &mut scene, "pocket near=3 mid=8, resname HEM");
    let added = operations(&outcome)
        .iter()
        .filter(|operation| **operation == "add")
        .count();
    assert_eq!(added, 6, "{:?}", operations(&outcome));
    assert_eq!(scene.spec().representations.len(), 6);
    assert_eq!(session.spec().layers.len(), 6);
    assert!(session.spec().focus.is_some());
    // The layers are ordinary: a later statement can hide one, and undo
    // removes the whole composition in one step.
    let name = session
        .spec()
        .layers
        .keys()
        .next()
        .map_or_else(|| panic!("a layer was registered"), ToString::to_string);
    run(&mut session, &mut scene, &format!("hide @{name}"));
    run(&mut session, &mut scene, "undo");
    run(&mut session, &mut scene, "undo");
    assert!(scene.spec().representations.is_empty());
}

#[test]
fn pocket_round_trips_through_its_canonical_text() {
    let source = "pocket near=3 mid=8 in one, resname HEM";
    let parsed = crate::Program::parse(source)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
        .statements()
        .first()
        .map(|statement| statement.command.clone());
    let Some(command @ Command::Pocket { .. }) = parsed else {
        panic!("pocket parses to its own command")
    };
    assert_eq!(command.to_string(), source);
}

#[test]
fn pocket_rejects_a_missing_subject_bad_radii_and_unordered_shells() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    assert_eq!(
        fail(&mut session, &mut scene, "pocket").kind,
        ErrorKind::Syntax
    );
    assert_eq!(
        fail(&mut session, &mut scene, "pocket near=-1, resname HEM").kind,
        ErrorKind::Syntax
    );
    assert_eq!(
        fail(&mut session, &mut scene, "pocket radius=3, resname HEM").kind,
        ErrorKind::Syntax
    );
    // near must stay inside mid; the scene layer owns that rule.
    assert_eq!(
        fail(&mut session, &mut scene, "pocket near=9 mid=3, resname HEM").kind,
        ErrorKind::Scene
    );
    assert!(scene.spec().representations.is_empty());
}
