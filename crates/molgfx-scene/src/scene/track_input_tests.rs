//! The track clock inputs author and lower through the one input registry.

use super::tests::structure;
use super::*;
use crate::{BoolExpr, Color, ColorExpr, ScalarExpr, VisualStyle, rep, sel};
use std::sync::Arc;

fn style_reading(name: &str) -> VisualStyle {
    VisualStyle {
        color: ColorExpr::Constant(Color::rgb(10, 20, 30)),
        opacity: ScalarExpr::input(name),
        visible: BoolExpr::Constant(true),
    }
}

#[test]
fn track_phase_and_track_time_author_as_named_renderer_inputs() {
    for name in ["track_phase", "track_time"] {
        let compiled = style_reading(name)
            .compile()
            .unwrap_or_else(|error| panic!("{name} must compile: {error}"));
        assert!(
            compiled.wgsl().contains(&format!("inputs.{name}")),
            "{name}"
        );
    }
}

#[test]
fn track_phase_and_track_time_lower_to_the_renderer_through_the_same_registry() {
    for name in ["track_phase", "track_time"] {
        let mut scene =
            Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
        // Authoring accepted the style above, so lowering must accept it too.
        if let Err(error) = scene.add(rep::cartoon(sel::all()).visual(style_reading(name))) {
            panic!("{name} must lower: {error}")
        }
    }
}

#[test]
fn an_unknown_input_is_refused_by_authoring_and_lowering_alike_naming_the_new_inputs() {
    let style = style_reading("track_phase_typo");
    let Err(authored) = style.compile() else {
        panic!("an unknown input must not compile")
    };
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let Err(lowered) = scene.add(rep::cartoon(sel::all()).visual(style)) else {
        panic!("an unknown input must not lower")
    };
    assert_eq!(authored.to_string(), lowered.to_string());
    assert!(authored.to_string().contains("track_phase"), "{authored}");
    assert!(authored.to_string().contains("track_time"), "{authored}");
}

fn frames(
    scene: &Scene,
    structure: crate::StructureId,
) -> (crate::TrajectoryFrame, crate::TrajectoryFrame) {
    let count = scene
        .molecular_pose(structure)
        .unwrap_or_else(|error| panic!("{error}"))
        .positions()
        .len();
    (
        crate::TrajectoryFrame::new(2, 1.0, Arc::from(vec![[0.0; 3]; count])),
        crate::TrajectoryFrame::new(5, 3.0, Arc::from(vec![[2.0; 3]; count])),
    )
}

#[test]
fn a_scene_morph_plays_between_its_frames_and_refuses_a_second_track() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let id = crate::StructureId::new(1);
    let (from, to) = frames(&scene, id);
    let mut timeline = molgfx_core::Timeline::new();
    scene
        .morph_timeline_trajectory(&mut timeline, id, from.clone(), to.clone(), 4.0)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply_timeline(&mut timeline, 2.0)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(segment) = scene
        .resolved()
        .structures()
        .next()
        .and_then(|(_, placed)| placed.trajectory())
    else {
        panic!("the morph installs a segment")
    };
    assert_eq!(segment.interpolation().to_bits(), 0.5f64.to_bits());
    assert_eq!(segment.start().index(), 2);
    assert_eq!(segment.end().index(), 5);
    assert!(
        scene
            .morph_timeline_trajectory(&mut timeline, id, from, to, 4.0)
            .is_err()
    );
}
