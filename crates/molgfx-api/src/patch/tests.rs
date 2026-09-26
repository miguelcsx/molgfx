use crate::appearance::tests::two_chains;
use crate::{Color, ColorSpec, Error, PatchError, PatchOperation, Scene, ScenePatch, color, rep};

fn scene_with_cartoon() -> (Scene, crate::RepresentationId) {
    let Ok(mut scene) = Scene::from_structure(&two_chains()) else {
        panic!("scene builds")
    };
    let Ok(id) = scene.add(rep::cartoon("protein")) else {
        panic!("cartoon adds")
    };
    (scene, id)
}

fn patch(scene: &Scene, operations: Vec<PatchOperation>) -> ScenePatch {
    ScenePatch {
        base_revision: scene.revision(),
        operations,
    }
}

#[test]
fn set_color_recolours_in_place_and_inverts() {
    let (mut scene, id) = scene_with_cartoon();
    let handle = scene.representation_handle(id);
    let base = scene.to_spec();
    let red = color::uniform(Color::rgb(255, 0, 0));
    let edit = patch(
        &scene,
        vec![PatchOperation::SetColor {
            id,
            color: red.clone(),
        }],
    );
    let Ok(inverse) = edit.inverse(&base) else {
        panic!("inverts")
    };
    assert!(scene.apply(&edit).is_ok());
    assert_eq!(scene.representation_handle(id), handle);
    let Some(physical) = handle.and_then(|handle| scene.resolved().representation(handle)) else {
        panic!("physical representation")
    };
    assert_eq!(
        physical.color,
        molgfx_core::ColorScheme::Uniform(molgfx_math::Rgba8::opaque(255, 0, 0))
    );
    let Some(spec) = scene.spec().representations.get(&id) else {
        panic!("spec")
    };
    assert_eq!(spec.color(), &red);
    assert!(scene.apply(&inverse).is_ok());
    assert_eq!(scene.spec().representations, base.representations);
}

#[test]
fn set_color_validates_before_mutating() {
    let (mut scene, id) = scene_with_cartoon();
    let before = scene.to_spec();
    let Ok(property) =
        serde_json::from_str::<crate::ScalarProperty>(r#"{"structure":1,"name":"absent"}"#)
    else {
        panic!("reference parses")
    };
    let bad = color::property(
        property,
        "no-such-ramp",
        [0.0, 1.0],
        None,
        Color::rgb(0, 0, 0),
    );
    let result = scene.apply(&patch(
        &scene,
        vec![PatchOperation::SetColor { id, color: bad }],
    ));
    assert!(result.is_err());
    assert_eq!(scene.spec(), &before);
}

#[test]
fn set_representation_target_rebuilds_only_the_retargeted_membership() {
    let (mut scene, id) = scene_with_cartoon();
    let handle = scene.representation_handle(id);
    let base = scene.to_spec();
    let edit = patch(
        &scene,
        vec![PatchOperation::SetRepresentationTarget {
            id,
            target: "chain A".into(),
        }],
    );
    let Ok(inverse) = edit.inverse(&base) else {
        panic!("inverts")
    };
    assert!(scene.apply(&edit).is_ok());
    assert_eq!(scene.representation_handle(id), handle);
    let rows = |scene: &Scene| {
        let Some(physical) = handle.and_then(|handle| scene.resolved().representation(handle))
        else {
            panic!("physical representation")
        };
        let Some(selection) = physical.selection() else {
            panic!("a molecular target")
        };
        let Some(rows) = scene.resolved().selection(selection) else {
            panic!("rows")
        };
        rows.count(9)
    };
    assert_eq!(rows(&scene), 4);
    assert!(scene.apply(&inverse).is_ok());
    assert_eq!(rows(&scene), 8);
    assert_eq!(scene.spec().representations, base.representations);
}

#[test]
fn a_malformed_target_fails_without_changing_the_scene() {
    let (mut scene, id) = scene_with_cartoon();
    let before = scene.to_spec();
    let result = scene.apply(&patch(
        &scene,
        vec![PatchOperation::SetRepresentationTarget {
            id,
            target: "chain (".into(),
        }],
    ));
    assert!(result.is_err());
    assert_eq!(scene.spec(), &before);
}

#[test]
fn new_operations_round_trip_through_json() {
    let (scene, id) = scene_with_cartoon();
    let edit = patch(
        &scene,
        vec![
            PatchOperation::SetColor {
                id,
                color: ColorSpec::SecondaryStructure,
            },
            PatchOperation::SetRepresentationTarget {
                id,
                target: "chain B".into(),
            },
            PatchOperation::AddAppearanceRule {
                id: crate::AppearanceRuleId(3),
                rule: crate::AppearanceRuleSpec::new(
                    crate::StructureId(1),
                    "resname HEM",
                    color::uniform(Color::rgb(255, 51, 102)),
                ),
            },
            PatchOperation::RemoveAppearanceRule {
                id: crate::AppearanceRuleId(3),
            },
        ],
    );
    let Ok(json) = edit.to_json() else {
        panic!("serializes")
    };
    assert!(json.contains(r#""op":"set_color""#));
    assert!(json.contains(r#""op":"set_representation_target""#));
    assert!(json.contains(r#""op":"add_appearance_rule""#));
    let Ok(parsed) = ScenePatch::from_json(&json) else {
        panic!("parses")
    };
    assert_eq!(parsed, edit);
}

#[test]
fn new_operations_reject_a_stale_revision() {
    let (mut scene, id) = scene_with_cartoon();
    let stale = ScenePatch {
        base_revision: scene.revision() + 7,
        operations: vec![PatchOperation::SetColor {
            id,
            color: ColorSpec::Chain,
        }],
    };
    assert!(matches!(
        scene.apply(&stale),
        Err(Error::Patch(PatchError::RevisionConflict { .. }))
    ));
}

#[test]
fn a_candidate_spec_applies_the_new_operations_without_a_renderer() {
    let (scene, id) = scene_with_cartoon();
    let edit = patch(
        &scene,
        vec![
            PatchOperation::SetColor {
                id,
                color: ColorSpec::Residue,
            },
            PatchOperation::AddAppearanceRule {
                id: crate::AppearanceRuleId(1),
                rule: crate::AppearanceRuleSpec::new(
                    crate::StructureId(1),
                    "chain A",
                    ColorSpec::Element,
                ),
            },
        ],
    );
    let Ok(candidate) = scene.spec().patched(&edit) else {
        panic!("patches")
    };
    assert_eq!(candidate.revision, scene.revision() + 1);
    assert_eq!(candidate.appearance.len(), 1);
}
