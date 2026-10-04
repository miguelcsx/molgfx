use super::tests::{fail, run};
use crate::{Command, ErrorKind, Session};
use molgfx_scene::{Color, DataSource, Scene, SegmentStyle, SegmentationSpec};

fn declaration() -> Command {
    Command::Segment {
        segmentation: SegmentationSpec {
            presentation: molgfx_scene::SegmentationPresentation::Surface,
            source: DataSource::new("labels"),
            dimensions: [2, 2, 2],
            voxel_to_world: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
            styles: vec![SegmentStyle {
                label: 42,
                color: Color::rgb(255, 0, 0),
                opacity: 1.0,
                visible: true,
            }],
        },
    }
}

#[test]
fn categorical_declarations_and_style_replacements_are_undoable_and_redoable() {
    let mut scene = Scene::empty();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, &declaration().to_string());
    let (&id, spec) = scene
        .spec()
        .segmentations
        .first_key_value()
        .expect("segmentation");
    let original = spec.clone();
    let text = format!("segment style {}:{} []", id.index, id.generation);
    run(&mut session, &mut scene, &text);
    assert!(scene.spec().segmentations[&id].styles.is_empty());
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().segmentations[&id], original);
    run(&mut session, &mut scene, "redo");
    assert!(scene.spec().segmentations[&id].styles.is_empty());
    run(&mut session, &mut scene, "undo");
    run(&mut session, &mut scene, "undo");
    assert!(scene.spec().segmentations.is_empty());
    run(&mut session, &mut scene, "redo");
    assert_eq!(scene.spec().segmentations[&id], original);
}

#[test]
fn invalid_or_stale_style_edits_do_not_commit_part_of_a_program() {
    let mut scene = Scene::empty();
    let mut session = Session::new(&scene);
    run(&mut session, &mut scene, &declaration().to_string());
    let id = *scene
        .spec()
        .segmentations
        .first_key_value()
        .expect("segmentation")
        .0;
    let before = scene.to_spec();
    let text = format!(
        "{}; segment style {}:{} []",
        declaration(),
        id.index,
        id.generation + 1
    );
    assert_eq!(fail(&mut session, &mut scene, &text).kind, ErrorKind::Scene);
    assert_eq!(scene.to_spec(), before);
    let text = format!(
        "segment style {}:{} [{{\"label\":42,\"color\":[1,2,3,255],\"opacity\":2.0,\"visible\":true}}]",
        id.index, id.generation
    );
    assert_eq!(fail(&mut session, &mut scene, &text).kind, ErrorKind::Scene);
    assert_eq!(scene.to_spec(), before);
}

#[test]
fn help_and_completion_publish_the_segmentation_verb_and_style_action() {
    assert!(
        crate::registry::VERBS
            .iter()
            .any(|verb| verb.name == "segment")
    );
    let scene = Scene::empty();
    let session = Session::new(&scene);
    assert!(
        session
            .completions("seg", 3)
            .iter()
            .any(|entry| entry.text == "segment")
    );
    assert!(
        session
            .completions("segment st", 10)
            .iter()
            .any(|entry| entry.text == "style")
    );
}
