//! Interaction channels, focus and the state bits they publish.

use super::tests::structure;
use super::*;
use crate::{rep, sel};

#[test]
fn an_interaction_state_style_reaches_the_renderer() {
    let style = crate::VisualStyle {
        color: crate::ColorExpr::Constant(crate::Color::rgb(255, 204, 0)),
        opacity: crate::ScalarExpr::from(1.0),
        visible: crate::BoolExpr::state("selected"),
    };
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    // Authoring accepted this style, so lowering must accept it too.
    if let Err(error) = scene.add(rep::cartoon(sel::all()).visual(style)) {
        panic!("an interaction-state style must lower: {error}")
    }
}

#[test]
fn a_style_may_read_a_channel_the_same_scene_declares() {
    let style = crate::VisualStyle {
        color: crate::ColorExpr::Constant(crate::Color::rgb(0, 0, 255)),
        opacity: crate::ScalarExpr::from(1.0),
        visible: crate::BoolExpr::state("pocket"),
    };
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    if let Err(error) = scene.set_interaction(
        crate::InteractionChannel::Custom("pocket".into()),
        Some(sel::all().into()),
    ) {
        panic!("declaring a channel must succeed: {error}")
    }
    if let Err(error) = scene.add(rep::cartoon(sel::all()).visual(style)) {
        panic!("a declared channel must lower: {error}")
    }
}

#[test]
fn an_undeclared_interaction_channel_is_rejected_with_the_known_names() {
    let style = crate::VisualStyle {
        color: crate::ColorExpr::Constant(crate::Color::rgb(0, 0, 255)),
        opacity: crate::ScalarExpr::from(1.0),
        visible: crate::BoolExpr::state("not_a_channel"),
    };
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let Err(error) = scene.add(rep::cartoon(sel::all()).visual(style)) else {
        panic!("an undeclared channel must not lower")
    };
    let message = error.to_string();
    assert!(message.contains("not_a_channel"), "{message}");
    assert!(message.contains("selected"), "{message}");
}

#[test]
fn an_unknown_renderer_input_is_rejected_at_authoring_time() {
    let style = crate::VisualStyle {
        color: crate::ColorExpr::Constant(crate::Color::rgb(0, 0, 0)),
        opacity: crate::ScalarExpr::input("frames_per_second"),
        visible: crate::BoolExpr::Constant(true),
    };
    let Err(error) = style.compile() else {
        panic!("an unknown input must not compile")
    };
    let message = error.to_string();
    assert!(message.contains("frames_per_second"), "{message}");
    assert!(message.contains("camera_distance"), "{message}");
}

#[test]
fn an_interaction_edit_writes_gpu_resident_state_bits() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::cartoon(sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.resolved().interaction_state_revision();

    if let Err(error) =
        scene.set_interaction(crate::InteractionChannel::Selected, Some(sel::all().into()))
    {
        panic!("selecting must apply: {error}")
    }

    let resolved = scene.resolved();
    assert_ne!(
        resolved.interaction_state_revision(),
        before,
        "a selection edit must advance the state revision"
    );
    let Some((handle, _)) = resolved.structures().next() else {
        panic!("the scene has a structure")
    };
    let Some(state) = resolved.interaction_state(handle) else {
        panic!("the structure has a state column")
    };
    let selected = molgfx_core::InteractionState::SELECTED.bits();
    assert!(
        state.values().iter().all(|word| word & selected != 0),
        "every selected atom carries the selected bit"
    );
}

#[test]
fn clearing_an_interaction_channel_clears_its_bits() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    if let Err(error) =
        scene.set_interaction(crate::InteractionChannel::Hovered, Some(sel::all().into()))
    {
        panic!("hovering must apply: {error}")
    }
    if let Err(error) = scene.set_interaction(crate::InteractionChannel::Hovered, None) {
        panic!("clearing must apply: {error}")
    }
    let resolved = scene.resolved();
    let Some((handle, _)) = resolved.structures().next() else {
        panic!("the scene has a structure")
    };
    let Some(state) = resolved.interaction_state(handle) else {
        panic!("the structure has a state column")
    };
    let hovered = molgfx_core::InteractionState::HOVERED.bits();
    assert!(
        state.values().iter().all(|word| word & hovered == 0),
        "clearing a channel must clear its bit"
    );
}

#[test]
fn focus_marks_its_subject_mutes_distant_context_and_publishes_bounds() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::cartoon(sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        scene.focus_bounds().is_none(),
        "a scene without a focus has no focus bounds"
    );

    if let Err(error) = scene.focus(sel::all()) {
        panic!("focusing must apply: {error}")
    }

    let Some(bounds) = scene.focus_bounds() else {
        panic!("a focused scene publishes focus bounds")
    };
    assert!(bounds.min.x.is_finite() && bounds.max.x.is_finite());

    let resolved = scene.resolved();
    let Some((handle, _)) = resolved.structures().next() else {
        panic!("the scene has a structure")
    };
    let Some(state) = resolved.interaction_state(handle) else {
        panic!("the structure has a state column")
    };
    let focused = molgfx_core::InteractionState::FOCUSED.bits();
    let muted = molgfx_core::InteractionState::MUTED.bits();
    assert!(
        state.values().iter().all(|word| word & focused != 0),
        "the focused subject carries the focused bit"
    );
    // Everything here is inside the context shell of itself, so nothing mutes.
    assert!(
        state.values().iter().all(|word| word & muted == 0),
        "atoms inside the focus context are not muted"
    );
}

#[test]
fn an_explicit_muted_channel_is_not_overridden_by_focus() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    if let Err(error) =
        scene.set_interaction(crate::InteractionChannel::Muted, Some(sel::none().into()))
    {
        panic!("muting must apply: {error}")
    }
    if let Err(error) = scene.focus(sel::all()) {
        panic!("focusing must apply: {error}")
    }
    let resolved = scene.resolved();
    let Some((handle, _)) = resolved.structures().next() else {
        panic!("the scene has a structure")
    };
    let Some(state) = resolved.interaction_state(handle) else {
        panic!("the structure has a state column")
    };
    let muted = molgfx_core::InteractionState::MUTED.bits();
    assert!(
        state.values().iter().all(|word| word & muted == 0),
        "an explicit empty muted channel stays empty"
    );
}

/// Two residues placed far enough apart that neither is in the other's context
/// shell, so focusing one must mute the other.
fn separated_residues() -> molframe::Structure {
    const PDB: &str = concat!(
        "ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N\n",
        "ATOM      2  CA  ALA A   1       1.500   0.000   0.000  1.00  0.00           C\n",
        "ATOM      3  N   GLY A   2      60.000   0.000   0.000  1.00  0.00           N\n",
        "ATOM      4  CA  GLY A   2      61.500   0.000   0.000  1.00  0.00           C\n",
        "END\n",
    );
    let result = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("pair.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

#[test]
fn focus_mutes_residues_outside_its_context_shell() {
    let mut scene =
        Scene::from_structure(&separated_residues()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::cartoon(sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));
    if let Err(error) = scene.focus(sel::resname().eq("ALA")) {
        panic!("focusing must apply: {error}")
    }

    let resolved = scene.resolved();
    let Some((handle, _)) = resolved.structures().next() else {
        panic!("the scene has a structure")
    };
    let Some(state) = resolved.interaction_state(handle) else {
        panic!("the structure has a state column")
    };
    let focused = molgfx_core::InteractionState::FOCUSED.bits();
    let muted = molgfx_core::InteractionState::MUTED.bits();
    let words = state.values();
    assert_eq!(words.len(), 4, "the fixture has four atoms");

    let focused_rows = words.iter().filter(|word| *word & focused != 0).count();
    let muted_rows = words.iter().filter(|word| *word & muted != 0).count();
    assert_eq!(focused_rows, 2, "the focused residue carries the focus bit");
    assert_eq!(
        muted_rows, 2,
        "the distant residue is muted as out-of-context"
    );
    // The focused subject is never also muted.
    assert!(
        words
            .iter()
            .all(|word| word & focused == 0 || word & muted == 0),
        "a focused atom must not also be muted"
    );
}

#[test]
fn focus_bounds_cover_only_the_focused_subject() {
    let mut scene =
        Scene::from_structure(&separated_residues()).unwrap_or_else(|error| panic!("{error}"));
    if let Err(error) = scene.focus(sel::resname().eq("ALA")) {
        panic!("focusing must apply: {error}")
    }
    let Some(bounds) = scene.focus_bounds() else {
        panic!("a focused scene publishes focus bounds")
    };
    // The distant residue sits at x = 60; focus bounds must not reach it.
    assert!(
        bounds.max.x < 30.0,
        "focus bounds reached the unfocused residue: {:?}",
        bounds.max
    );
}
