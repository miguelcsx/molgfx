//! A representation-only patch must leave the scene exactly as a full
//! resolution of the same specification would.

use crate::appearance::tests::two_chains;
use crate::{
    AppearanceRuleSpec, Color, PatchOperation, RepresentationId, Scene, ScenePatch, StructureId,
    color, rep,
};
use std::collections::BTreeMap;

fn scene() -> Scene {
    match Scene::from_structure(&two_chains()) {
        Ok(scene) => scene,
        Err(error) => panic!("scene builds: {error}"),
    }
}

fn added(scene: &mut Scene, selection: &str) -> RepresentationId {
    match scene.add(rep::cartoon(selection)) {
        Ok(id) => id,
        Err(error) => panic!("representation adds: {error}"),
    }
}

fn removed(scene: &mut Scene, id: RepresentationId) {
    if let Err(error) = scene.transaction(|transaction| transaction.remove(id)) {
        panic!("representation removes: {error}")
    }
}

/// Everything a representation draws, keyed by semantic identity and with the
/// scene-local handles resolved to the rows they stand for.
fn digest(scene: &Scene) -> BTreeMap<RepresentationId, String> {
    scene
        .representations
        .iter()
        .map(|(id, handle)| {
            let Some(drawn) = scene.resolved.representation(*handle) else {
                panic!("a live handle resolves")
            };
            let rows = match drawn.target {
                molgfx_core::RepresentationTarget::Selection(selection) => {
                    format!("{:?}", scene.resolved.selection(selection))
                }
                other => format!("{other:?}"),
            };
            let drawn = format!(
                "{:?}",
                (
                    drawn.kind,
                    &drawn.color,
                    &drawn.color_overlay,
                    drawn.material,
                    &drawn.params,
                    drawn.visible,
                    drawn.visual.is_some(),
                )
            );
            (*id, format!("{drawn} {rows}"))
        })
        .collect()
}

fn fresh(scene: &Scene) -> Scene {
    let structures = BTreeMap::from([(StructureId(1), two_chains())]);
    match Scene::from_spec(scene.spec().clone(), structures) {
        Ok(scene) => scene,
        Err(error) => panic!("specification resolves: {error}"),
    }
}

fn assert_equivalent(scene: &Scene) {
    let full = fresh(scene);
    assert_eq!(digest(scene), digest(&full));
    assert_eq!(
        scene.resolved.representation_count(),
        full.resolved.representation_count()
    );
    assert_eq!(
        scene.resolved.selection_count(),
        full.resolved.selection_count()
    );
    assert_eq!(scene.visuals.len(), full.visuals.len());
    assert_eq!(scene.next_representation, full.next_representation);
}

#[test]
fn add_remove_and_replace_match_a_full_resolution() {
    let mut scene = scene();
    let first = added(&mut scene, "chain A");
    let second = added(&mut scene, "chain B");
    let third = added(&mut scene, "resname HEM");
    assert_equivalent(&scene);
    removed(&mut scene, second);
    assert_equivalent(&scene);
    let Some(mut replacement) = scene.spec().representations.get(&third).cloned() else {
        panic!("the representation exists")
    };
    replacement.common.opacity = 0.4;
    replacement.common.visible = false;
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::ReplaceRepresentation {
            id: first,
            representation: replacement,
        }],
    };
    assert!(scene.apply(&patch).is_ok());
    assert_equivalent(&scene);
    let again = added(&mut scene, "chain B");
    assert_ne!(again, second);
    assert_equivalent(&scene);
}

#[test]
fn untouched_representations_are_not_rebuilt() {
    let mut scene = scene();
    let first = added(&mut scene, "chain A");
    assert!(scene.set_opacity(first, 0.5).is_ok());
    let Some(handle) = scene.representations.get(&first).copied() else {
        panic!("the representation resolves")
    };
    let before = scene.resolved.representation_content_revision(handle);
    assert!(before.is_some_and(|revision| revision > 0));
    let second = added(&mut scene, "chain B");
    assert_eq!(
        scene.resolved.representation_content_revision(handle),
        before
    );
    removed(&mut scene, second);
    assert_eq!(scene.representations.get(&first), Some(&handle));
    assert_eq!(
        scene.resolved.representation_content_revision(handle),
        before
    );
}

#[test]
fn a_selection_is_released_with_its_last_representation_only() {
    let mut scene = scene();
    let first = added(&mut scene, "chain A");
    let second = added(&mut scene, "chain A");
    let other = added(&mut scene, "chain B");
    let shared = scene.resolved.selection_count();
    removed(&mut scene, first);
    assert_eq!(scene.resolved.selection_count(), shared);
    removed(&mut scene, second);
    assert_eq!(scene.resolved.selection_count(), shared - 1);
    removed(&mut scene, other);
    assert_eq!(scene.resolved.selection_count(), shared - 2);
    assert_equivalent(&scene);
}

#[test]
fn a_new_representation_shares_its_structure_overlay() {
    let mut scene = scene();
    let _ = added(&mut scene, "chain A");
    let rule = AppearanceRuleSpec::new(
        StructureId(1),
        "chain B",
        color::uniform(Color::rgb(255, 0, 0)),
    );
    let staged = scene.transaction(|transaction| transaction.add_appearance_rule(rule).map(|_| ()));
    assert!(staged.is_ok());
    let fresh_one = added(&mut scene, "protein");
    assert_equivalent(&scene);
    let Some(handle) = scene.representations.get(&fresh_one) else {
        panic!("the representation resolves")
    };
    let overlay = scene
        .resolved
        .representation(*handle)
        .map(|drawn| drawn.color_overlay.is_some());
    assert_eq!(overlay, Some(true));
}

#[test]
fn a_failing_patch_leaves_the_scene_untouched() {
    let mut scene = scene();
    let kept = added(&mut scene, "chain A");
    let before = digest(&scene);
    let revision = scene.revision();
    let selections = scene.resolved.selection_count();
    let result = scene.add(rep::cartoon("no_such_keyword ((("));
    assert!(result.is_err());
    assert_eq!(scene.revision(), revision);
    assert_eq!(digest(&scene), before);
    assert_eq!(scene.resolved.selection_count(), selections);
    assert!(scene.representations.contains_key(&kept));
}
