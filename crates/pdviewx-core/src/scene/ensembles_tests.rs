use crate::{Ensemble, Scene};
use std::sync::Arc;

#[test]
fn scene_ensembles_validate_members_and_keep_stable_handles() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::new();
    let first = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let second = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let ensemble = Ensemble::new(Arc::from([first, second]), &[0.7, 0.3], "caller:poses")
        .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .add_ensemble(ensemble)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        scene.ensemble(handle).map(|value| value.members().len()),
        Some(2)
    );
    assert_eq!(scene.ensembles().count(), 1);
    assert!(scene.remove_ensemble(handle).is_some());
    assert!(scene.ensemble(handle).is_none());

    let stale = Ensemble::new(Arc::from([first, second]), &[0.5, 0.5], "caller:stale")
        .unwrap_or_else(|error| panic!("{error}"));
    scene.remove_structure(first);
    assert!(matches!(
        scene.add_ensemble(stale),
        Err(crate::CoreError::StaleHandle)
    ));
}
