use super::*;
use crate::EntityKind;
use crate::handle::RawHandle;

fn owner() -> StructureHandle {
    StructureHandle(RawHandle::new_for_test(2, 0))
}

fn anchor(x: f32) -> AnnotationAnchor {
    match AnnotationAnchor::world(Vec3::new(x, 0.0, 0.0)) {
        Ok(anchor) => anchor,
        Err(error) => panic!("fixture anchor builds: {error}"),
    }
}

#[test]
fn annotations_validate_text_markers_and_entity_provenance() {
    let entity = EntityRef {
        structure: owner(),
        kind: EntityKind::Atom,
        index: 7,
    };
    let entity_anchor = match AnnotationAnchor::entity(Vec3::X, entity) {
        Ok(anchor) => anchor,
        Err(error) => panic!("entity anchor builds: {error}"),
    };
    let note = match Annotation::note(owner(), entity_anchor, "catalytic contact") {
        Ok(note) => note.with_priority(8),
        Err(error) => panic!("note builds: {error}"),
    };
    assert_eq!(note.kind(), AnnotationKind::Note);
    assert_eq!(
        note.anchor().and_then(AnnotationAnchor::source_entity),
        Some(entity)
    );
    assert_eq!(note.text(), "catalytic contact");
    assert_eq!(note.priority(), 8);
    assert!(matches!(
        Annotation::note(owner(), anchor(0.0), "   "),
        Err(CoreError::InvalidAnnotation { .. })
    ));
    assert!(matches!(
        Annotation::marker(
            owner(),
            anchor(0.0),
            MarkerStyle {
                radius_pixels: f32::NAN,
                ..MarkerStyle::default()
            }
        ),
        Err(CoreError::InvalidAnnotation { .. })
    ));
}

#[test]
fn measurement_values_are_supplied_validated_and_preformatted_once() {
    let distance = match Measurement::distance(
        owner(),
        [anchor(0.0), anchor(2.417)],
        2.417,
        "pdbiox:distance/v1",
    ) {
        Ok(value) => value,
        Err(error) => panic!("distance builds: {error}"),
    };
    assert_eq!(distance.kind(), MeasurementKind::Distance);
    assert_eq!(distance.anchors().len(), 2);
    assert_eq!(distance.label(), "2.42 Å");
    assert_eq!(distance.provenance(), "pdbiox:distance/v1");
    let angle = Measurement::angle(
        owner(),
        [anchor(0.0), anchor(1.0), anchor(2.0)],
        181.0,
        "caller",
    );
    assert!(matches!(angle, Err(CoreError::InvalidAnnotation { .. })));
    let dihedral = match Measurement::dihedral(
        owner(),
        [anchor(0.0), anchor(1.0), anchor(2.0), anchor(3.0)],
        -179.95,
        "caller:torsion",
    ) {
        Ok(value) => value,
        Err(error) => panic!("dihedral builds: {error}"),
    };
    assert_eq!(dihedral.label(), "-179.9°");
}
