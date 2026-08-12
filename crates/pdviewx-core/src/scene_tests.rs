use super::*;
use crate::fixture;
use crate::representation::RepresentationKind;
use crate::selection::AtomSelection;

fn scene() -> Scene {
    match Scene::from_structure(&fixture::structure()) {
        Ok(scene) => scene,
        Err(e) => panic!("fixture scene must build: {e}"),
    }
}

#[test]
fn a_scene_built_from_a_structure_exposes_its_atoms() {
    let s = scene();
    let Some(atoms) = s.first_atoms() else {
        panic!("scene has a structure")
    };
    assert_eq!(atoms.len(), 8);
}

#[test]
fn representing_a_selection_succeeds_for_supported_kinds_only() {
    let mut s = scene();
    let sel = s.add_selection(AtomSelection::All);
    assert!(s.represent(sel, RepresentationKind::Spacefill).is_ok());
    assert!(s.represent(sel, RepresentationKind::BallAndStick).is_ok());
    let Err(err) = s.represent(sel, RepresentationKind::Volume) else {
        panic!("volume is not drawable yet")
    };
    assert_eq!(err.code(), "PDVIEWX-E0042");
}

#[test]
fn a_stale_selection_handle_is_refused_with_its_code() {
    let mut a = scene();
    let mut b = scene();
    let foreign = b.add_selection(AtomSelection::All);
    // Handle from another scene: same slot space shape, but scene `a` has no
    // entry there yet, so resolution fails.
    let Err(err) = a.represent(foreign, RepresentationKind::Spacefill) else {
        panic!("foreign handle must not resolve")
    };
    assert_eq!(err.code(), "PDVIEWX-E0041");
    let _ = b.representation_count();
}

#[test]
fn representation_edits_bump_the_revision_and_reads_do_not() {
    let mut s = scene();
    let sel = s.add_selection(AtomSelection::All);
    let Ok(rep) = s.represent(sel, RepresentationKind::Spacefill) else {
        panic!("spacefill applies")
    };
    let after_add = s.representation_revision();
    let _ = s.representation(rep);
    let _ = s.representations().count();
    assert_eq!(s.representation_revision(), after_add);
    s.hide(rep);
    assert!(s.representation_revision() > after_add);
}

#[test]
fn removing_a_structure_stales_its_handle() {
    let mut s = Scene::new();
    let Ok(h) = s.add_structure(&fixture::structure()) else {
        panic!("fixture places")
    };
    assert!(s.structure(h).is_some());
    assert!(s.remove_structure(h).is_some());
    assert!(s.structure(h).is_none());
}

#[test]
fn the_world_bound_covers_the_placed_structure() {
    let s = scene();
    let aabb = s.world_aabb();
    assert!(!aabb.is_empty());
    // The sulfate sulfur sits at x = 8; the bound must reach it.
    assert!(aabb.max.x >= 8.0);
}
