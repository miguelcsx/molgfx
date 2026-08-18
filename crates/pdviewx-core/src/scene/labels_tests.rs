use crate::{
    Annotation, AnnotationAnchor, AtomSelection, EntityKind, EntityRef, Measurement, Scene, fixture,
};
use pdviewx_math::Vec3;

fn scene() -> Scene {
    Scene::from_structure(&fixture::structure()).unwrap_or_else(|error| panic!("{error}"))
}

fn owner(scene: &Scene) -> crate::StructureHandle {
    scene
        .structures()
        .next()
        .map_or_else(|| panic!("fixture owner exists"), |(handle, _)| handle)
}

fn anchor(position: Vec3) -> AnnotationAnchor {
    AnnotationAnchor::world(position).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn annotations_and_measurements_share_stable_pick_rows_without_type_confusion() {
    let mut scene = scene();
    let owner = owner(&scene);
    let note = Annotation::note(owner, anchor(Vec3::ZERO), "active-site note")
        .unwrap_or_else(|error| panic!("{error}"));
    let note = scene
        .add_annotation(note)
        .unwrap_or_else(|error| panic!("{error}"));
    let distance = Measurement::distance(
        owner,
        [anchor(Vec3::ZERO), anchor(Vec3::X)],
        1.0,
        "caller:test",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let distance = scene
        .add_measurement(distance)
        .unwrap_or_else(|error| panic!("{error}"));
    let note_entity = EntityRef {
        structure: owner,
        kind: EntityKind::Label,
        index: Scene::annotation_row(note),
    };
    let measurement_entity = EntityRef {
        structure: owner,
        kind: EntityKind::Label,
        index: Scene::measurement_row(distance),
    };
    assert_eq!(
        scene
            .annotation_for_entity(note_entity)
            .map(|(handle, value)| (handle, value.text())),
        Some((note, "active-site note"))
    );
    assert!(scene.measurement_for_entity(note_entity).is_none());
    assert_eq!(
        scene
            .measurement_for_entity(measurement_entity)
            .map(|(handle, value)| (handle, value.label())),
        Some((distance, "1.00 Å"))
    );
    assert!(scene.annotation_for_entity(measurement_entity).is_none());
}

#[test]
fn label_edits_and_removals_advance_one_independent_revision() {
    let mut scene = scene();
    let owner = owner(&scene);
    let selection = scene.add_selection(AtomSelection::All);
    let region =
        Annotation::region(owner, selection, "protein").unwrap_or_else(|error| panic!("{error}"));
    let before = scene.label_revision();
    let handle = scene
        .add_annotation(region)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(scene.label_revision() > before);
    let after_add = scene.label_revision();
    scene
        .annotation_mut(handle)
        .unwrap_or_else(|| panic!("annotation resolves"))
        .set_visible(false);
    assert!(scene.label_revision() > after_add);
    assert_eq!(scene.annotations().count(), 1);
    assert_eq!(scene.measurements().count(), 0);
    assert!(scene.remove_annotation(handle).is_some());
    assert!(scene.annotation(handle).is_none());
}

#[test]
fn absent_owners_regions_and_source_structures_are_stale_errors() {
    let mut scene = scene();
    let owner = owner(&scene);
    let stale_selection = crate::SelectionHandle(crate::handle::RawHandle::new_for_test(999, 0));
    let region = Annotation::region(owner, stale_selection, "gone")
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(matches!(
        scene.add_annotation(region),
        Err(crate::CoreError::StaleHandle)
    ));
    let stale_entity = EntityRef {
        structure: crate::StructureHandle(crate::handle::RawHandle::new_for_test(999, 0)),
        kind: EntityKind::Atom,
        index: 0,
    };
    let anchored = AnnotationAnchor::entity(Vec3::ZERO, stale_entity)
        .unwrap_or_else(|error| panic!("{error}"));
    let note = Annotation::note(owner, anchored, "stale").unwrap_or_else(|error| panic!("{error}"));
    assert!(matches!(
        scene.add_annotation(note),
        Err(crate::CoreError::StaleHandle)
    ));
}
