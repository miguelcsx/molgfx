//! Consumer-visible current-selection history and transaction boundaries.

use super::tests::{fail, run, scene};
use crate::Session;

#[test]
fn selection_replacement_and_clear_are_undoable() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let revision = scene.revision();
    run(&mut session, &mut scene, "selection chain A");
    assert_eq!(scene.revision(), revision + 1);
    let selected = scene.spec().selected.clone();
    assert!(selected.is_some());
    run(&mut session, &mut scene, "selection");
    assert_eq!(scene.spec().selected, None);
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().selected, selected);
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().selected, None);
    run(&mut session, &mut scene, "redo");
    assert_eq!(scene.spec().selected, selected);
}

#[test]
fn selection_is_available_to_later_statements_without_self_reference() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "selection chain A; selection $sel and name CA; show points, $sel",
    );
    let selected = scene.spec().selected.as_ref().expect("selected atoms");
    assert!(!selected.source().contains("$sel"));
    let target = scene
        .spec()
        .representations
        .values()
        .next()
        .expect("points layer")
        .target();
    assert_eq!(target, selected);
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().selected, None);
    assert!(scene.spec().representations.is_empty());
}

#[test]
fn later_failure_rolls_back_selection_and_existing_history() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "selection chain B");
    let before = scene.spec().clone();
    let revision = scene.revision();
    fail(
        &mut session,
        &mut scene,
        "selection chain A; show points, $missing",
    );
    assert_eq!(scene.revision(), revision);
    assert_eq!(scene.spec(), &before);
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().selected, None);
}
