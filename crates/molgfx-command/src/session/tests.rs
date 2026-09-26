use crate::{Command, ErrorKind, Outcome, Session, SessionSpec};
use molgfx_api::{PatchOperation, Scene};

/// Two protein chains of two residues each, and one haem iron in chain C.
const CIF: &str = "\
data_two
_entry.id two
loop_
_entity.id
_entity.type
1 polymer
2 non-polymer
loop_
_entity_poly.entity_id
_entity_poly.type
1 'polypeptide(L)'
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.auth_seq_id
_atom_site.auth_comp_id
_atom_site.auth_asym_id
_atom_site.auth_atom_id
ATOM 1 N N GLY A 1 1 0.0 0.0 0.0 1 GLY A N
ATOM 2 C CA GLY A 1 1 1.4 0.0 0.0 1 GLY A CA
ATOM 3 N N GLY A 1 2 3.0 0.5 0.0 2 GLY A N
ATOM 4 C CA GLY A 1 2 4.4 0.5 0.0 2 GLY A CA
ATOM 5 N N GLY B 1 1 0.0 5.0 0.0 1 GLY B N
ATOM 6 C CA GLY B 1 1 1.4 5.0 0.0 1 GLY B CA
ATOM 7 N N GLY B 1 2 3.0 5.5 0.0 2 GLY B N
ATOM 8 C CA GLY B 1 2 4.4 5.5 0.0 2 GLY B CA
HETATM 9 FE FE HEM C 2 . 2.0 2.5 0.0 101 HEM C FE
";

pub(super) fn structure() -> molframe::Structure {
    match molframe::read_bytes(
        CIF.as_bytes().to_vec(),
        Some("two.cif"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(error) => panic!("fixture parses: {error:?}"),
    }
}

fn scene() -> Scene {
    match Scene::from_structure(&structure()) {
        Ok(scene) => scene,
        Err(error) => panic!("scene builds: {error}"),
    }
}

fn run(session: &mut Session, scene: &mut Scene, source: &str) -> Outcome {
    match session.execute_text(scene, source) {
        Ok(outcome) => outcome,
        Err(errors) => panic!("{source:?} runs: {}", errors.render(source)),
    }
}

fn fail(session: &mut Session, scene: &mut Scene, source: &str) -> crate::CommandError {
    match session.execute_text(scene, source) {
        Ok(outcome) => panic!("{source:?} should fail, got {outcome:?}"),
        Err(errors) => match errors.0.into_iter().next() {
            Some(error) => error,
            None => panic!("an error"),
        },
    }
}

fn operations(outcome: &Outcome) -> Vec<&'static str> {
    outcome
        .patch
        .iter()
        .flat_map(|patch| &patch.operations)
        .map(|operation| match operation {
            PatchOperation::AddRepresentation { .. } => "add",
            PatchOperation::RemoveRepresentation { .. } => "remove",
            PatchOperation::SetRepresentationTarget { .. } => "target",
            PatchOperation::SetColor { .. } => "color",
            PatchOperation::AddAppearanceRule { .. } => "add_rule",
            PatchOperation::ReplaceAppearanceRule { .. } => "replace_rule",
            PatchOperation::RemoveAppearanceRule { .. } => "remove_rule",
            PatchOperation::SetVisibility { .. } => "visibility",
            PatchOperation::SetOpacity { .. } => "opacity",
            PatchOperation::SetFocus { .. } => "focus",
            _ => "other",
        })
        .collect()
}

#[test]
fn show_is_idempotent_and_duplicate_is_explicit() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    assert_eq!(
        operations(&run(&mut session, &mut scene, "show cartoon, protein")),
        ["add"]
    );
    let again = run(&mut session, &mut scene, "show cartoon, protein");
    assert_eq!(operations(&again), ["visibility"]);
    assert_eq!(scene.spec().representations.len(), 1);
    run(&mut session, &mut scene, "show cartoon duplicate, protein");
    assert_eq!(scene.spec().representations.len(), 2);
    assert!(
        session
            .spec()
            .layers
            .keys()
            .any(|name| name.as_str() == "cartoon_2")
    );
}

#[test]
fn a_redefined_selection_retargets_what_uses_it() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select site, chain A; show spacefill as site, $site; color red, $site",
    );
    let outcome = run(&mut session, &mut scene, "select site, chain B");
    assert_eq!(operations(&outcome), ["target", "replace_rule"]);
    let target = scene
        .spec()
        .representations
        .values()
        .next()
        .map(|representation| representation.target().source().to_owned());
    assert_eq!(target.as_deref(), Some("chain B"));
}

#[test]
fn a_selection_through_another_follows_both() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select core, chain A; select near, $core or resname HEM; show lines, $near",
    );
    let outcome = run(&mut session, &mut scene, "select core, chain B");
    assert_eq!(operations(&outcome), ["target"]);
}

#[test]
fn a_cycle_between_selections_is_refused() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "select a, chain A; select b, $a");
    let error = fail(&mut session, &mut scene, "select a, $b");
    assert_eq!(error.kind, ErrorKind::Cycle);
    assert!(error.message.contains("a -> b -> a"), "{}", error.message);
}

#[test]
fn an_unknown_selection_is_located_and_suggested() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "select pocket, resname HEM");
    let source = "show cartoon, $pockt";
    let error = fail(&mut session, &mut scene, source);
    assert_eq!(error.kind, ErrorKind::UnknownSymbol);
    assert_eq!(error.suggestion.as_deref(), Some("pocket"));
    assert_eq!(
        error.span.map(|span| &source[span.start..span.end]),
        Some("$pockt")
    );
}

#[test]
fn a_selection_used_as_a_layer_is_the_wrong_kind() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "select pocket, resname HEM");
    assert_eq!(
        fail(&mut session, &mut scene, "hide @pocket").kind,
        ErrorKind::WrongSymbolKind
    );
}

#[test]
fn a_selection_in_use_cannot_be_removed() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select pocket, resname HEM; show spacefill, $pocket",
    );
    let error = fail(&mut session, &mut scene, "unselect pocket");
    assert_eq!(error.kind, ErrorKind::InUse);
    run(
        &mut session,
        &mut scene,
        "remove @spacefill; unselect pocket",
    );
    assert!(session.spec().selections.is_empty());
}

#[test]
fn a_failing_statement_leaves_scene_and_session_unchanged() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let before = (scene.spec().clone(), session.spec().clone());
    let error = fail(
        &mut session,
        &mut scene,
        "select a, chain A; show cartoon, $a; hide @nothing",
    );
    assert_eq!(error.statement, Some(2));
    assert_eq!((scene.spec().clone(), session.spec().clone()), before);
}

#[test]
fn colouring_a_layer_is_a_local_colour_edit() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "show cartoon as main, protein");
    let outcome = run(&mut session, &mut scene, "color chain, @main");
    assert_eq!(operations(&outcome), ["color"]);
}

#[test]
fn colouring_a_query_again_replaces_its_rule() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "show cartoon, protein; color red, chain A",
    );
    let outcome = run(&mut session, &mut scene, "color blue, chain A");
    assert_eq!(operations(&outcome), ["remove_rule", "add_rule"]);
    assert_eq!(scene.spec().appearance.len(), 1);
    run(&mut session, &mut scene, "uncolor");
    assert!(scene.spec().appearance.is_empty());
}

#[test]
fn undo_and_redo_restore_scene_and_names() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let empty = scene.spec().representations.clone();
    run(
        &mut session,
        &mut scene,
        "select a, chain A; show cartoon, $a",
    );
    let drawn = scene.spec().representations.clone();
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().representations, empty);
    assert!(session.spec().selections.is_empty());
    run(&mut session, &mut scene, "redo");
    assert_eq!(scene.spec().representations, drawn);
    assert_eq!(session.spec().selections.len(), 1);
    assert_eq!(
        fail(&mut session, &mut scene, "redo").kind,
        ErrorKind::History
    );
}

#[test]
fn several_undos_are_one_scene_revision() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "show cartoon, protein");
    run(&mut session, &mut scene, "show spacefill, resname HEM");
    let revision = scene.revision();
    let outcome = run(&mut session, &mut scene, "undo; undo");
    assert_eq!(outcome.revision, revision + 1);
    assert!(scene.spec().representations.is_empty());
}

#[test]
fn an_outside_edit_clears_history() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, "show cartoon as main, protein");
    let id = session.spec().layers.values().map(|layer| layer.id).next();
    if let Some(id) = id
        && let Err(error) = scene.set_opacity(id, 0.5)
    {
        panic!("outside edit: {error}");
    }
    assert_eq!(
        fail(&mut session, &mut scene, "undo").kind,
        ErrorKind::StaleSession
    );
    run(&mut session, &mut scene, "hide @main");
}

#[test]
fn several_structures_must_be_named() {
    let mut scene = scene();
    if let Err(error) = scene.add_structure(&structure()) {
        panic!("second structure: {error}");
    }
    let mut session = Session::new(&scene);
    assert_eq!(
        fail(&mut session, &mut scene, "show cartoon, protein").kind,
        ErrorKind::AmbiguousStructure
    );
    run(&mut session, &mut scene, "show cartoon in s2, protein");
    let structure = session
        .spec()
        .layers
        .values()
        .map(|layer| layer.structure.get())
        .next();
    assert_eq!(structure, Some(2));
}

#[test]
fn focus_follows_its_selection() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select site, resname HEM; focus $site",
    );
    let outcome = run(&mut session, &mut scene, "select site, chain A");
    assert_eq!(operations(&outcome), ["focus"]);
    run(&mut session, &mut scene, "unfocus");
    assert!(scene.spec().focus.is_none());
}

#[test]
fn a_session_spec_round_trips_through_json() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select site, resname HEM; show spacefill as lig, $site; color red, $site",
    );
    let json = session
        .spec()
        .to_json()
        .unwrap_or_else(|error| panic!("{error}"));
    let spec = SessionSpec::from_json(&json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(&spec, session.spec());
    let mut restored = Session::from_spec(spec, &scene);
    run(&mut restored, &mut scene, "select site, chain A");
}

#[test]
fn typed_commands_run_without_text() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let command: Command = serde_json::from_str(
        r#"{"command":"show","form":{"form":"cartoon"},"target":{"query":"protein"}}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let outcome = session
        .run(&mut scene, command)
        .unwrap_or_else(|errors| panic!("{errors}"));
    assert_eq!(operations(&outcome), ["add"]);
    assert_eq!(session.history(), ["show cartoon, protein"]);
}

#[test]
fn completion_offers_what_the_position_accepts() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select pocket, resname HEM; show cartoon as main, protein",
    );
    let texts = |text: &str| -> Vec<String> {
        session
            .completions(text, text.len())
            .into_iter()
            .map(|completion| completion.text)
            .collect()
    };
    assert!(texts("sh").contains(&"show".to_owned()));
    assert!(texts("show car").contains(&"cartoon".to_owned()));
    assert!(texts("show cartoon wi").contains(&"width=".to_owned()));
    assert!(texts("show cartoon, $po").contains(&"$pocket".to_owned()));
    assert!(texts("hide @m").contains(&"@main".to_owned()));
    assert!(texts("color re").contains(&"red".to_owned()));
}

#[test]
fn explanations_name_declared_and_resolved_targets() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    run(
        &mut session,
        &mut scene,
        "select site, resname HEM; show spacefill as lig, $site",
    );
    let selection = session
        .explain_selection("site")
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(selection.contains("used by: layer '@lig'"), "{selection}");
    let layer = session
        .explain_layer("lig")
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(layer.contains("declared: $site"), "{layer}");
    assert!(layer.contains("resname HEM"), "{layer}");
}
