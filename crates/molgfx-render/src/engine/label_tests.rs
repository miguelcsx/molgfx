use super::tests::{camera, engine, structure};
use molgfx_core::{Annotation, AnnotationAnchor, Measurement, Scene};
use molgfx_math::Vec3;

fn labelled_scene() -> Scene {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let anchor =
        |position| AnnotationAnchor::world(position).unwrap_or_else(|error| panic!("{error}"));
    scene
        .add_annotation(
            Annotation::note(owner, anchor(Vec3::new(0.0, 1.0, 0.0)), "ACTIVE SITE")
                .unwrap_or_else(|error| panic!("{error}"))
                .with_priority(20),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .add_measurement(
            Measurement::distance(
                owner,
                [
                    anchor(Vec3::new(-1.0, 0.0, 0.0)),
                    anchor(Vec3::new(1.0, 0.0, 0.0)),
                ],
                2.0,
                "test:molframe",
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    scene
}

#[test]
fn all_label_records_compact_to_one_indirect_draw() {
    let scene = labelled_scene();
    let mut engine = engine();
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let draws = engine
        .device
        .log
        .indirect_draws
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    // The glyph, guide and marker kinds share one compacted table but each
    // draws with its own specialized pipeline, so there are three indirect
    // draws over the same arguments buffer.
    assert_eq!(draws.len(), 3);
    let batch = draws.first().map(|(args, _)| *args);
    assert!(draws.iter().all(|(args, _)| Some(*args) == batch));
    let dispatches = engine
        .device
        .log
        .dispatches
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(dispatches.as_slice(), &[(1, 1, 1)]);
}

#[test]
fn unchanged_labels_upload_only_frame_uniforms() {
    let scene = labelled_scene();
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
    engine
        .render(&scene, &camera())
        .unwrap_or_else(|error| panic!("{error}"));
    let writes = engine
        .device
        .log
        .writes
        .lock()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(writes.len() - before, 1, "only frame uniforms change");
}
