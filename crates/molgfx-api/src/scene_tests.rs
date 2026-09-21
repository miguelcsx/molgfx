use super::*;
use crate::{rep, sel};

fn structure() -> molframe::Structure {
    const PDB: &str =
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n";
    let result = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("one.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

#[test]
fn transactions_advance_once_and_apply_every_edit() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let id = scene
        .add(rep::cartoon(sel::protein()))
        .unwrap_or_else(|error| panic!("{error}"));
    let revision = scene.revision();
    let patch = scene
        .transaction(|edit| {
            edit.set_opacity(id, 0.4);
            edit.set_visible(id, false);
            Ok(())
        })
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(patch.base_revision, revision);
    assert_eq!(scene.revision(), revision + 1);
    let Some(representation) = scene.spec().representations.get(&id) else {
        panic!("representation exists")
    };
    assert!((representation.opacity() - 0.4).abs() < f32::EPSILON);
    assert!(!representation.visible);
}

#[test]
fn a_conflicting_patch_changes_nothing() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let before = scene.to_spec();
    let result = scene.apply(&ScenePatch::empty(99));
    assert!(matches!(
        result,
        Err(Error::Patch(PatchError::RevisionConflict { .. }))
    ));
    assert_eq!(scene.spec(), &before);
}

#[test]
fn an_empty_patch_is_a_true_no_op() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let before = scene.to_spec();
    let patch = ScenePatch::empty(scene.revision());
    let inverse = patch
        .inverse(&before)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.spec(), &before);
    assert_eq!(inverse.base_revision, before.revision);
    assert!(inverse.operations.is_empty());
}

#[test]
fn canonical_json_contains_no_runtime_handles() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::spacefill("all"))
        .unwrap_or_else(|error| panic!("{error}"));
    let json = scene
        .spec()
        .to_json()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!json.contains("generation"));
    assert!(!json.contains("row"));
    let roundtrip = SceneSpec::from_json(&json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(roundtrip, *scene.spec());
}

#[test]
fn inverse_patch_restores_the_base_semantics() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let id = scene
        .add(rep::spacefill("all"))
        .unwrap_or_else(|error| panic!("{error}"));
    let base = scene.to_spec();
    let patch = scene
        .transaction(|edit| {
            edit.set_opacity(id, 0.2);
            edit.set_visible(id, false);
            Ok(())
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let inverse = patch
        .inverse(&base)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply(&inverse)
        .unwrap_or_else(|error| panic!("{error}"));
    let restored = scene
        .spec()
        .representations
        .get(&id)
        .unwrap_or_else(|| panic!("representation exists"));
    let expected = base
        .representations
        .get(&id)
        .unwrap_or_else(|| panic!("representation exists"));
    assert_eq!(restored, expected);
}

#[test]
fn inverse_tracks_intermediate_state_for_add_then_remove() {
    let scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let base = scene.to_spec();
    let id = RepresentationId::new(9);
    let representation = rep::points("all")
        .structure(StructureId::new(1))
        .into_spec();
    let patch = ScenePatch {
        base_revision: base.revision,
        operations: vec![
            PatchOperation::AddRepresentation { id, representation },
            PatchOperation::RemoveRepresentation { id },
        ],
    };
    let inverse = patch
        .inverse(&base)
        .unwrap_or_else(|error| panic!("{error}"));
    let applied = base
        .patched(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    let restored = applied
        .patched(&inverse)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(restored.representations, base.representations);
}

#[test]
fn interaction_edits_only_advance_interaction_revision() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let before = scene.spec().revisions;
    scene
        .set_interaction(InteractionChannel::Selected, Some(sel::heavy().into()))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.spec().revisions.interaction, before.interaction + 1);
    assert_eq!(scene.spec().revisions.appearance, before.appearance);
    assert_eq!(scene.spec().revisions.coordinates, before.coordinates);
}

#[test]
fn equal_queries_share_one_resolved_selection() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::spacefill(sel::heavy()))
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::points(sel::heavy()))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.selections.len(), 1);
}

#[test]
fn an_invalid_interaction_query_is_atomic() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let before = scene.to_spec();
    let result = scene.focus("this is not a molecular query");
    assert!(result.is_err());
    assert_eq!(scene.spec(), &before);
}

#[test]
fn a_constant_visual_edit_changes_only_appearance() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let id = scene
        .add(rep::spacefill("all"))
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.spec().revisions;
    let visual = crate::VisualStyle::new(
        crate::Color::rgb(8, 16, 32),
        0.5,
        crate::BoolExpr::Constant(true),
    );
    scene
        .set_visual(id, Some(visual))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.spec().revisions.appearance, before.appearance + 1);
    assert_eq!(scene.spec().revisions.selection, before.selection);
}

#[test]
fn parameterized_visuals_lower_once_per_named_parameter() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let parameter = crate::Parameter::new("opacity", 0.6_f32);
    let style = crate::VisualStyle::new(
        crate::Color::rgb(20, 40, 60),
        crate::ScalarExpr::from(parameter.clone()) * crate::ScalarExpr::from(parameter),
        crate::BoolExpr::Constant(true),
    );
    let id = scene
        .add(rep::spacefill("all").visual(style))
        .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .representations
        .get(&id)
        .copied()
        .unwrap_or_else(|| panic!("representation exists"));
    let visual = scene
        .resolved
        .representation(handle)
        .and_then(|representation| representation.visual.as_ref())
        .unwrap_or_else(|| panic!("visual exists"));
    assert_eq!(visual.parameters().len(), 1);
}

#[test]
fn parameter_edits_keep_the_program_and_change_only_appearance() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let parameter = crate::Parameter::new("opacity", 0.6_f32);
    let style = crate::VisualStyle::new(
        crate::Color::rgb(20, 40, 60),
        crate::ScalarExpr::from(parameter.clone()),
        crate::BoolExpr::Constant(true),
    );
    let id = scene
        .add(rep::spacefill("all").visual(style))
        .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .representations
        .get(&id)
        .copied()
        .unwrap_or_else(|| panic!("representation exists"));
    let before = scene.spec().revisions;
    let program = scene
        .resolved
        .representation(handle)
        .and_then(|representation| representation.visual.as_ref())
        .map_or_else(
            || panic!("visual exists"),
            |visual| visual.program().clone(),
        );
    scene
        .set_parameter(id, &parameter, 0.25)
        .unwrap_or_else(|error| panic!("{error}"));
    let visual = scene
        .resolved
        .representation(handle)
        .and_then(|representation| representation.visual.as_ref())
        .unwrap_or_else(|| panic!("visual exists"));
    assert_eq!(visual.program(), &program);
    assert!((visual.parameters()[0][0] - 0.25).abs() < f32::EPSILON);
    assert_eq!(scene.spec().revisions.appearance, before.appearance + 1);
    assert_eq!(scene.spec().revisions.selection, before.selection);
}

#[test]
fn invalid_parameter_types_are_rejected_atomically() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let parameter = crate::Parameter::new("opacity", 0.6_f32);
    let style = crate::VisualStyle::new(
        crate::Color::rgb(20, 40, 60),
        crate::ScalarExpr::from(parameter),
        crate::BoolExpr::Constant(true),
    );
    let id = scene
        .add(rep::spacefill("all").visual(style))
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.to_spec();
    let wrong_type = crate::Parameter::new("opacity", crate::Color::rgb(1, 2, 3));
    assert!(
        scene
            .set_parameter(id, &wrong_type, crate::Color::rgb(4, 5, 6))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
}

#[test]
fn inverse_parameter_patch_restores_the_declared_default() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let parameter = crate::Parameter::new("opacity", 0.6_f32);
    let style = crate::VisualStyle::new(
        crate::Color::rgb(20, 40, 60),
        crate::ScalarExpr::from(parameter.clone()),
        crate::BoolExpr::Constant(true),
    );
    let id = scene
        .add(rep::spacefill("all").visual(style))
        .unwrap_or_else(|error| panic!("{error}"));
    let base = scene.to_spec();
    let patch = ScenePatch {
        base_revision: base.revision,
        operations: vec![PatchOperation::SetParameter {
            id,
            name: parameter.name().into(),
            value: Some(crate::ParameterValue::Scalar(0.2)),
        }],
    };
    let inverse = patch
        .inverse(&base)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply(&inverse)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        scene.spec().representations.get(&id),
        base.representations.get(&id)
    );
}

#[test]
fn inverse_visual_replacement_restores_parameter_overrides() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let parameter = crate::Parameter::new("opacity", 0.6_f32);
    let style = crate::VisualStyle::new(
        crate::Color::rgb(20, 40, 60),
        crate::ScalarExpr::from(parameter.clone()),
        crate::BoolExpr::Constant(true),
    );
    let id = scene
        .add(rep::spacefill("all").visual(style))
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .set_parameter(id, &parameter, 0.25)
        .unwrap_or_else(|error| panic!("{error}"));
    let base = scene.to_spec();
    let replacement = crate::VisualStyle::new(
        crate::Color::rgb(80, 90, 100),
        1.0,
        crate::BoolExpr::Constant(true),
    );
    let patch = ScenePatch {
        base_revision: base.revision,
        operations: vec![PatchOperation::SetVisual {
            id,
            visual: Some(replacement),
        }],
    };
    let inverse = patch
        .inverse(&base)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply(&inverse)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        scene.spec().representations.get(&id),
        base.representations.get(&id)
    );
}

#[test]
fn color_and_vector_parameters_share_one_transaction_revision() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let color = crate::Parameter::new("tint", crate::Color::rgb(10, 20, 30));
    let vector = crate::Parameter::new("direction", [0.0_f32, 1.0, 0.0]);
    let opacity = crate::VectorExpr::from(vector.clone())
        .dot(crate::VectorExpr::from([0.0, 1.0, 0.0]))
        .clamp(0.0, 1.0);
    let style = crate::VisualStyle::new(
        crate::ColorExpr::from(color.clone()),
        opacity,
        crate::BoolExpr::Constant(true),
    );
    let id = scene
        .add(rep::spacefill("all").visual(style))
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.revision();
    scene
        .transaction(|edit| {
            edit.set_parameter(id, &color, crate::Color::rgb(120, 80, 40));
            edit.set_parameter(id, &vector, [1.0, 0.0, 0.0]);
            Ok(())
        })
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.revision(), before + 1);
    let representation = scene
        .spec()
        .representations
        .get(&id)
        .unwrap_or_else(|| panic!("representation exists"));
    assert_eq!(representation.parameters.len(), 2);
}

#[test]
fn camera_edits_advance_only_the_view_revision() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let before = scene.spec().revisions;
    let camera = molgfx_math::Camera {
        eye: molgfx_math::Vec3::new(1.0, 2.0, 8.0),
        target: molgfx_math::Vec3::new(1.0, 2.0, 3.0),
        up: molgfx_math::Vec3::Y,
        projection: molgfx_math::Projection::Perspective {
            fov_y: 0.7,
            aspect: 1.5,
            near: 0.1,
            far: 100.0,
        },
    };
    scene
        .set_camera(Some(camera))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.spec().camera, Some(camera));
    assert_eq!(scene.spec().revisions.view, before.view + 1);
    assert_eq!(scene.spec().revisions.appearance, before.appearance);
    assert_eq!(scene.spec().revisions.placement, before.placement);
}

#[test]
fn multi_structure_representations_require_and_intern_by_asset_identity() {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let second = scene
        .add_structure(&source)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(scene.add(rep::points(sel::heavy())).is_err());
    let _ = scene
        .add(rep::points(sel::heavy()).structure(StructureId::new(1)))
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::spacefill(sel::heavy()).structure(second))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.selections.len(), 2);
}
