use super::tests::{engine, render_ok, structure};
use crate::testing::Locked;
use molgfx_core::{AtomSelection, RepresentationKind, Scene};

fn one_representation_scene() -> Scene {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(value) => value,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    if let Err(error) = scene.represent(selection, RepresentationKind::Spacefill) {
        panic!("fixture representation builds: {error}");
    }
    scene
}

#[test]
fn a_different_scene_invalidates_equal_revision_values() {
    let mut engine = engine();
    let first = one_representation_scene();
    let second = one_representation_scene();
    assert_ne!(first.cache_identity(), second.cache_identity());
    render_ok(&mut engine, &first);
    let mut writes = engine.device.log.writes.locked();
    writes.clear();
    drop(writes);
    render_ok(&mut engine, &second);
    let writes = engine.device.log.writes.locked();
    assert!(
        writes.len() > 1,
        "scene replacement uploads resident data in addition to frame uniforms"
    );
}
