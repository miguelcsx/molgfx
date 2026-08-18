use super::tests::{camera, engine, structure};
use pdviewx_core::{
    AtomProperty, AtomPropertyMeaning, AtomSelection, ColorScheme, PropertyAppearance,
    RepresentationKind, ScalarFieldSemantics, Scene,
};
use std::sync::Arc;

fn property(
    owner: pdviewx_core::StructureHandle,
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
