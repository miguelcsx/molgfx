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

#[test]
fn keyword_aliases_select_the_same_atoms_as_the_canonical_word() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    for (alias, canonical) in [
        ("ligands", "ligand"),
        ("hetatm", "hetero"),
        ("solvent", "water"),
        ("glycans", "saccharide"),
        ("ions", "ion"),
        ("resi 1", "resid 1"),
        ("b_factor > 5", "bfactor > 5"),
    ] {
        let alias = session.resolve_selection(alias).expect("alias resolves");
        let canonical = session
            .resolve_selection(canonical)
            .expect("canonical resolves");
        assert_eq!(alias, canonical);
    }
    run(&mut session, &mut scene, "show lines, ligands and resi 2");
}

#[test]
fn an_unknown_keyword_is_named_with_nearest_words() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let source = "show lines, liganz";
    let error = fail(&mut session, &mut scene, source);
    assert!(error.message.contains("'liganz'"), "{}", error.message);
    assert_eq!(error.suggestion.as_deref(), Some("ligand"));
    let span = error.span.expect("located");
    assert_eq!(&source[span.start..span.end], "liganz");
    let resolved = session
        .resolve_selection("liganz")
        .expect_err("unknown keyword");
    assert!(resolved.0[0].message.contains("'liganz'"));
}

#[test]
fn a_metric_colour_with_no_target_colours_every_layer() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "show cartoon; show lines, ligand");
    for spelling in ["b_factor", "bfactor", "bfac", "metric:b_factor", "plddt"] {
        run(&mut session, &mut scene, &format!("color {spelling}"));
    }
}

#[test]
fn a_metric_colour_on_part_of_a_layer_explains_the_working_form() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "show cartoon");
    let error = fail(&mut session, &mut scene, "color b_factor, protein");
    assert!(
        error.message.contains("continuous colour"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains("color b_factor, @"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains("color=b_factor"),
        "{}",
        error.message
    );
    let error = fail(&mut session, &mut scene, "color bfactr");
    assert_eq!(error.suggestion.as_deref(), Some("bfactor"));
}
