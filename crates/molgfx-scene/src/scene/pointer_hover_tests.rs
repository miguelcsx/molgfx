use super::*;
use crate::{InteractionChannel, PickKind, PickResult, sel};

fn atom_pick(scene: &Scene, row: u64) -> ResolvedPick {
    let (_, placed) = scene
        .resolved
        .structures()
        .next()
        .expect("fixture structure");
    scene
        .resolve_pick(&PickResult {
            kind: PickKind::Atom,
            dataset: Some(placed.dataset_id().get()),
            chunk: Some(0),
            row: Some(row),
            volume_label: None,
            segmentation: None,
            source_id: None,
        })
        .expect("fixture pick")
}

#[test]
fn transient_hover_preserves_selection_and_authored_state_and_repeated_picks_settle() {
    let mut scene = fixture();
    scene
        .set_interaction(InteractionChannel::Selected, Some(sel::all().into()))
        .expect("selected");
    scene
        .set_interaction(InteractionChannel::Hovered, Some(sel::all().into()))
        .expect("authored hover");
    let authored = scene.to_spec().to_json().expect("portable scene");
    let pick = atom_pick(&scene, 0);
    assert!(scene.set_pointer_hover(Some(&pick)).expect("hover"));
    let revision = scene.resolved.interaction_state_revision();
    assert!(!scene.set_pointer_hover(Some(&pick)).expect("repeat"));
    assert_eq!(revision, scene.resolved.interaction_state_revision());
    let (handle, _) = scene.resolved.structures().next().expect("structure");
    let words = scene
        .resolved
        .interaction_state(handle)
        .expect("states")
        .values();
    assert!(
        words
            .iter()
            .all(|word| word & molgfx_core::InteractionState::SELECTED.bits() != 0)
    );
    assert_eq!(
        words
            .iter()
            .filter(|word| *word & molgfx_core::InteractionState::HOVERED.bits() != 0)
            .count(),
        1
    );
    assert_eq!(
        authored,
        scene.to_spec().to_json().expect("portable state preserved")
    );
    assert!(
        scene
            .set_pointer_hover(None)
            .expect("restore authored hover")
    );
    assert!(!scene.set_pointer_hover(None).expect("repeated restoration"));
    let words = scene
        .resolved
        .interaction_state(handle)
        .expect("states")
        .values();
    assert!(
        words
            .iter()
            .all(|word| word & molgfx_core::InteractionState::HOVERED.bits() != 0)
    );
}

#[test]
fn stale_or_outside_hover_picks_leave_existing_state_unchanged() {
    let mut scene = fixture();
    let mut pick = atom_pick(&scene, 0);
    let ResolvedPick::Atom(atom) = &mut pick else {
        panic!("atom")
    };
    atom.atom_index = u32::MAX;
    let revision = scene.resolved.interaction_state_revision();
    assert!(scene.set_pointer_hover(Some(&pick)).is_err());
    let ResolvedPick::Atom(atom) = &mut pick else {
        panic!("atom")
    };
    atom.atom_index = 0;
    atom.topology_revision = u64::MAX;
    assert!(scene.set_pointer_hover(Some(&pick)).is_err());
    assert_eq!(revision, scene.resolved.interaction_state_revision());
}

fn fixture() -> Scene {
    let source = b"ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N\nATOM      2  CA  ALA A   1       1.500   0.000   0.000  1.00  0.00           C\nEND\n";
    let (source, _) = molframe::read_bytes(
        source.to_vec(),
        Some("hover.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("fixture reads");
    Scene::from_structure(&source).expect("fixture scene")
}
