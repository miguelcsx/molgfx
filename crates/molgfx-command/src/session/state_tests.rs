use super::state::State;
use super::tests::{scene, structure};
use molgfx_scene::Scene;

fn scene_of(count: usize) -> Scene {
    let mut scene = scene();
    for _ in 1..count {
        if let Err(error) = scene.add_structure(&structure()) {
            panic!("structure adds: {error}");
        }
    }
    scene
}

#[test]
fn sync_names_every_structure_once() {
    let scene = scene_of(40);
    let mut state = State::default();
    state.sync(scene.spec());
    assert_eq!(state.spec.structures.len(), 40);
    let named: std::collections::BTreeSet<_> = state.spec.structures.values().collect();
    assert_eq!(named.len(), 40);
}

#[test]
fn sync_keeps_existing_names_and_drops_removed_structures() {
    let scene = scene_of(6);
    let mut state = State::default();
    state.sync(scene.spec());
    let before = state.spec.structures.clone();
    state.sync(scene.spec());
    assert_eq!(state.spec.structures, before);
    let smaller = scene_of(3);
    state.sync(smaller.spec());
    assert_eq!(state.spec.structures.len(), 3);
    assert!(
        state
            .spec
            .structures
            .iter()
            .all(|(name, id)| before.get(name) == Some(id))
    );
}
