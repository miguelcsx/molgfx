use super::*;
use molgfx_math::Mat4;

fn placed_pair() -> (Scene, [StructureHandle; 2]) {
    let structure = crate::fixture::structure();
    let mut scene = Scene::new();
    let left = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let right = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(placed) = scene.structure_mut(right) else {
        panic!("right placement resolves")
    };
    placed.model_to_world = Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0));
    (scene, [left, right])
}

#[test]
fn caller_correspondence_drives_reversible_world_space_difference_views() {
    let (mut scene, structures) = placed_pair();
    let view = scene
        .render_difference(
            structures,
            &[AtomCorrespondence { left: 0, right: 0 }],
            "caller:alignment/v1",
            DifferenceStyle {
                representation: RepresentationKind::Spacefill,
                ..DifferenceStyle::default()
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert!((view.maximum_displacement - 2.0).abs() < 1.0e-6);
    for (side, attribute) in view.attributes.into_iter().enumerate() {
        let values = scene
            .attribute(attribute)
            .and_then(|attribute| match attribute.values() {
                AttributeValues::Scalar(values) => Some(values.as_ref()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("side {side} attribute resolves"));
        assert!((values[0] - 2.0).abs() < 1.0e-6);
        let descriptor = scene.attribute(attribute).map_or_else(
            || panic!("side {side} descriptor resolves"),
            AttributeColumn::descriptor,
        );
        assert_eq!(descriptor.quantity(), Some("displacement"));
        assert_eq!(descriptor.unit(), Some("angstrom"));
        assert_eq!(descriptor.provenance(), Some("caller:alignment/v1"));
        let representation = scene
            .representation(view.representations[side])
            .unwrap_or_else(|| panic!("side {side} representation resolves"));
        assert!(representation.visual.is_some());
        assert!(
            scene
                .selection_for(view.selections[side], structures[side])
                .is_some()
        );
    }
}

#[test]
fn repeated_correspondence_is_rejected_before_composition() {
    let (mut scene, structures) = placed_pair();
    let repeated = scene.render_difference(
        structures,
        &[
            AtomCorrespondence { left: 0, right: 0 },
            AtomCorrespondence { left: 0, right: 0 },
        ],
        "caller",
        DifferenceStyle::default(),
    );
    assert!(matches!(repeated, Err(CoreError::InvalidDifference { .. })));
    assert_eq!(scene.representation_count(), 0);
}

#[test]
fn a_style_outside_the_unit_interval_is_rejected_before_composition() {
    let (mut scene, structures) = placed_pair();
    let rejected = scene.render_difference(
        structures,
        &[AtomCorrespondence { left: 0, right: 0 }],
        "caller",
        DifferenceStyle {
            context_opacity: 1.0,
            ..DifferenceStyle::default()
        },
    );
    assert!(matches!(rejected, Err(CoreError::InvalidDifference { .. })));
    assert_eq!(scene.representation_count(), 0);
}
