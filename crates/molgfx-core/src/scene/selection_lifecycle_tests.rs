use crate::{AtomSelection, CoreError, MolecularSource, Representation, Scene};

fn scene() -> (Scene, crate::StructureHandle) {
    let mut scene = Scene::new();
    let source = MolecularSource::from_molframe(&crate::fixture::structure());
    let Ok(structure) = scene.add_source(source) else {
        panic!("fixture binds")
    };
    (scene, structure)
}

#[test]
fn a_query_selection_carries_its_fingerprint() {
    let (mut scene, structure) = scene();
    let Ok(handle) = scene.add_query_selection(structure, AtomSelection::All, 42) else {
        panic!("selection stores")
    };
    assert_eq!(scene.selection_fingerprint(handle), Some(42));
}

#[test]
fn a_selection_is_released_only_once_nothing_draws_it() {
    let (mut scene, structure) = scene();
    let Ok(drawn) = scene.add_query_selection(structure, AtomSelection::All, 1) else {
        panic!("selection stores")
    };
    let Ok(_) = scene.represent(drawn, Representation::spacefill()) else {
        panic!("representation binds")
    };
    assert!(matches!(
        scene.remove_selection(drawn),
        Err(CoreError::InvalidSelection { .. })
    ));
    let Ok(spare) = scene.add_query_selection(structure, AtomSelection::Empty, 2) else {
        panic!("selection stores")
    };
    assert!(scene.remove_selection(spare).is_ok());
    assert!(matches!(
        scene.remove_selection(spare),
        Err(CoreError::StaleHandle)
    ));
}
