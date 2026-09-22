use super::Scene;
use crate::{AtomSelection, RepresentationKind, RepresentationTarget};

#[test]
fn prepared_appearance_edits_do_not_invalidate_slot_membership() {
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let Ok(representation) = scene.represent(selection, RepresentationKind::Spacefill) else {
        panic!("spacefill applies")
    };
    let membership = scene.representation_membership_revision();
    let Some(mut replacement) = scene.representation(representation).cloned() else {
        panic!("representation resolves")
    };
    replacement.material.opacity = 0.4;
    if let Err(error) = scene.replace_representations(vec![(representation, replacement)]) {
        panic!("prepared edit applies: {error}")
    }
    assert_eq!(scene.representation_membership_revision(), membership);
}

#[test]
fn prepared_target_edits_invalidate_slot_membership_once() {
    let mut scene = Scene::new();
    let first = scene.add_selection(AtomSelection::All);
    let second = scene.add_selection(AtomSelection::Sparse(vec![0]));
    let Ok(representation) = scene.represent(first, RepresentationKind::Spacefill) else {
        panic!("spacefill applies")
    };
    let membership = scene.representation_membership_revision();
    let Some(mut replacement) = scene.representation(representation).cloned() else {
        panic!("representation resolves")
    };
    replacement.target = RepresentationTarget::Selection(second);
    if let Err(error) = scene.replace_representations(vec![(representation, replacement)]) {
        panic!("prepared edit applies: {error}")
    }
    assert_eq!(
        scene.representation_membership_revision(),
        membership.wrapping_add(1)
    );
}
