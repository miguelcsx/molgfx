use super::tests::{camera, engine, structure};
use molgfx_core::{
    AtomProperty, AtomPropertyMeaning, AtomSelection, ColorScheme, PropertyAppearance,
    RepresentationKind, ScalarFieldSemantics, ScalarRamp, Scene, VisualProgramBuilder, VisualStyle,
};
use std::sync::Arc;

fn property(
    owner: molgfx_core::StructureHandle,
    name: &'static str,
    values: [f32; 3],
) -> AtomProperty {
    AtomProperty::new(
        owner,
        name,
        Arc::from(values),
        AtomPropertyMeaning::Confidence,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn uncertainty_appearance_routes_atoms_through_transparency_and_tracks_its_property() {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let selected = scene.add_selection(AtomSelection::All);
    let representation = scene
        .represent(selected, RepresentationKind::Spacefill)
        .unwrap_or_else(|error| panic!("{error}"));
    let confidence = scene
        .add_atom_property(property(owner, "confidence", [0.1, 0.5, 0.9]))
        .unwrap_or_else(|error| panic!("{error}"));
    let appearance = PropertyAppearance::confidence(confidence, [0.0, 1.0])
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(representation) = scene.representation_mut(representation) else {
        panic!("representation resolves")
    };
    representation.appearance = Some(appearance);

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(engine.scene_gpu.has_translucency());
    assert_eq!(engine.scene_gpu.atom_draws(false).count(), 0);
    assert_eq!(engine.scene_gpu.atom_draws(true).count(), 1);

    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    scene
        .replace_atom_property(confidence, property(owner, "confidence", [0.9, 0.5, 0.1]))
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    assert!(after > before + 1, "dependent appearance records repack");
}

#[test]
fn replacing_one_property_recolors_only_its_dependent_slot() {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let selected = scene.add_selection(AtomSelection::All);
    let representation = scene
        .represent(selected, RepresentationKind::Spacefill)
        .unwrap_or_else(|error| panic!("{error}"));
    let confidence = scene
        .add_atom_property(property(owner, "confidence", [0.1, 0.5, 0.9]))
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(value) = scene.atom_property(confidence) else {
        panic!("confidence resolves")
    };
    let scheme = ColorScheme::property(confidence, value);
    let Some(representation) = scene.representation_mut(representation) else {
        panic!("representation resolves")
    };
    representation.color = scheme;

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());

    scene
        .replace_atom_property(confidence, property(owner, "confidence", [0.9, 0.5, 0.1]))
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after_recolor = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    assert!(after_recolor > before + 1, "dependent records are repacked");

    scene
        .add_atom_property(property(owner, "unrelated", [1.0, 2.0, 3.0]))
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        writes.len() - after_recolor,
        1,
        "only frame uniforms change"
    );
}

#[test]
fn visual_properties_upload_once_for_all_dependent_slots() {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let selected = scene.add_selection(AtomSelection::All);
    let first = scene
        .represent(selected, RepresentationKind::Spacefill)
        .unwrap_or_else(|error| panic!("{error}"));
    let second = scene
        .represent(selected, RepresentationKind::Points)
        .unwrap_or_else(|error| panic!("{error}"));
    let confidence = scene
        .add_atom_property(property(owner, "confidence", [0.1, 0.5, 0.9]))
        .unwrap_or_else(|error| panic!("{error}"));
    let style = VisualStyle::color_by_property(confidence, ScalarRamp::sequential([0.0, 1.0]))
        .unwrap_or_else(|error| panic!("{error}"));
    for handle in [first, second] {
        let Some(representation) = scene.representation_mut(handle) else {
            panic!("representation resolves")
        };
        representation.visual = Some(style.clone());
    }
    let source_pointer = scene
        .atom_property(confidence)
        .map_or(0, |column| column.values().as_ptr() as usize);

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let direct_writes = engine.device.log.writes.lock().map_or_else(
        |error| panic!("{error}"),
        |writes| {
            writes
                .iter()
                .filter(|(_, _, bytes, pointer)| *bytes == 12 && *pointer == source_pointer)
                .count()
        },
    );
    assert_eq!(direct_writes, 1, "one shared property upload is expected");

    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let steady_state_writes = engine.device.log.writes.lock().map_or_else(
        |error| panic!("{error}"),
        |writes| {
            writes
                .iter()
                .filter(|(_, _, bytes, pointer)| *bytes == 12 && *pointer == source_pointer)
                .count()
        },
    );
    assert_eq!(steady_state_writes, 1, "unchanged frames do not re-upload");
}

#[test]
fn visual_time_updates_only_the_presentation_uniform() {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let selected = scene.add_selection(AtomSelection::All);
    let handle = scene
        .represent(selected, RepresentationKind::Spacefill)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut builder = VisualProgramBuilder::new();
    let time = builder
        .time()
        .unwrap_or_else(|error| panic!("time input should build: {error}"));
    builder
        .set_opacity(time)
        .unwrap_or_else(|error| panic!("opacity should build: {error}"));
    let style = VisualStyle::new(
        builder
            .finish()
            .unwrap_or_else(|error| panic!("program should build: {error}")),
    );
    let Some(representation) = scene.representation_mut(handle) else {
        panic!("representation resolves")
    };
    representation.visual = Some(style);

    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    scene
        .set_presentation_time(0.5)
        .unwrap_or_else(|error| panic!("{error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let after = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());
    assert_eq!(after - before, 2, "frame and visual uniforms change");
}

#[test]
fn uniform_visual_parameter_updates_only_the_resolved_config_without_rebinding() {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let selected = scene.add_selection(AtomSelection::All);
    let mut builder = VisualProgramBuilder::new();
    let (_, opacity) = builder
        .scalar_parameter(0.25)
        .unwrap_or_else(|error| panic!("parameter should build: {error}"));
    builder
        .set_opacity(opacity)
        .unwrap_or_else(|error| panic!("opacity should build: {error}"));
    let style = VisualStyle::new(
        builder
            .finish()
            .unwrap_or_else(|error| panic!("program should build: {error}")),
    );
    let handle = scene
        .represent(
            selected,
            molgfx_core::Representation::spacefill().visual(style),
        )
        .unwrap_or_else(|error| panic!("representation should build: {error}"));
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let before = engine
        .device
        .log
        .writes
        .lock()
        .map_or_else(|error| panic!("{error}"), |writes| writes.len());

    scene
        .set_visual_scalar_parameter(handle, 0, 0.75)
        .unwrap_or_else(|error| panic!("parameter should update: {error}"));
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));

    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    let changed = &writes[before..];
    assert_eq!(
        changed.len(),
        2,
        "only frame and the once-resolved uniform config change: {changed:?}"
    );
    assert_eq!(
        changed
            .iter()
            .filter(|(_, _, bytes, _)| *bytes == 16)
            .count(),
        0,
        "a uniform program has no per-entity or fragment parameter upload"
    );
}
