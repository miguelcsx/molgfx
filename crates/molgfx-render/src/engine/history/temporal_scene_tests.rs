use super::TemporalSceneKey;
use molgfx_core::{AtomSelection, RepresentationKind, Scene};

#[test]
fn replacing_a_form_in_the_same_scene_discards_incompatible_history() {
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let first = scene
        .represent(selection, RepresentationKind::Spacefill)
        .unwrap();
    let before = TemporalSceneKey::from_scene(&scene);
    scene.remove_representation(first);
    scene
        .represent(selection, RepresentationKind::Lines)
        .unwrap();
    assert_ne!(before, TemporalSceneKey::from_scene(&scene));
}

#[test]
fn appearance_and_visibility_edits_discard_incompatible_history() {
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selection, RepresentationKind::Spacefill)
        .unwrap();
    let before = TemporalSceneKey::from_scene(&scene);
    scene
        .representation_mut(handle)
        .unwrap()
        .params
        .radius_scale = 0.5;
    let styled = TemporalSceneKey::from_scene(&scene);
    assert_ne!(before, styled);
    scene.hide(handle);
    assert_ne!(styled, TemporalSceneKey::from_scene(&scene));
}

#[test]
fn a_still_scene_preserves_history() {
    let scene = Scene::new();
    assert_eq!(
        TemporalSceneKey::from_scene(&scene),
        TemporalSceneKey::from_scene(&scene)
    );
}

#[test]
fn a_presented_form_change_restarts_accumulation_on_its_first_frame() {
    use crate::engine::tests::{camera, engine, structure};
    let mut engine = engine();
    let mut scene = Scene::from_structure(&structure()).unwrap();
    let selection = scene.add_selection(AtomSelection::All);
    let first = scene
        .represent(selection, RepresentationKind::Spacefill)
        .unwrap();
    engine.prepare_frame(&scene, &camera()).unwrap();
    engine.prepare_frame(&scene, &camera()).unwrap();
    assert!(engine.temporal.prepared_samples() > 1);
    scene.remove_representation(first);
    scene
        .represent(selection, RepresentationKind::BallAndStick)
        .unwrap();
    engine.prepare_frame(&scene, &camera()).unwrap();
    assert_eq!(engine.temporal.prepared_samples(), 1);
}

#[test]
fn bonds_submit_capsules_without_spheres_or_lone_atom_markers() {
    use crate::engine::tests::{camera, engine, structure};
    let mut engine = engine();
    let mut scene = Scene::from_structure(&structure()).unwrap();
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, RepresentationKind::Bonds)
        .unwrap();
    engine.prepare_frame(&scene, &camera()).unwrap();
    assert_eq!(engine.scene_gpu.atom_draws(false).count(), 0);
    assert_eq!(engine.scene_gpu.bond_draws(false).count(), 1);
    assert_eq!(engine.scene_gpu.point_draws(false).count(), 0);
}
