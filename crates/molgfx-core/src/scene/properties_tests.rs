use crate::{AtomProperty, AtomPropertyMeaning, CoreError, ScalarFieldSemantics, Scene};
use std::sync::Arc;

#[test]
fn interpolating_a_property_frame_lerps_values_and_keeps_nan_missing() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let count = usize::try_from(placed.atoms.len()).map_or(0, |value| value);
    let property = AtomProperty::new(
        owner,
        "energy",
        Arc::from(vec![0.0; count]),
        AtomPropertyMeaning::Charge,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .add_atom_property(property)
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.property_content_revision(handle);

    let mut start = vec![0.0_f32; count];
    let mut end = vec![4.0_f32; count];
    start[1] = f32::NAN; // missing in the earlier frame stays missing
    end[2] = f32::NAN; // missing in the later frame stays missing
    scene
        .interpolate_atom_property(handle, &start, &end, 0.25)
        .unwrap_or_else(|error| panic!("{error}"));

    let Some(updated) = scene.atom_property(handle) else {
        panic!("property")
    };
    let values = updated.values();
    assert!((values[0] - 1.0).abs() < 1e-5); // lerp(0, 4, 0.25)
    assert!(values[1].is_nan());
    assert!(values[2].is_nan());
    assert!(scene.property_content_revision(handle) > before);
}

#[test]
fn interpolating_a_property_frame_rejects_bad_inputs() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let count = usize::try_from(placed.atoms.len()).map_or(0, |value| value);
    let handle = scene
        .add_atom_property(
            AtomProperty::new(
                owner,
                "energy",
                Arc::from(vec![0.0; count]),
                AtomPropertyMeaning::Charge,
                ScalarFieldSemantics::UncalibratedRank,
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let row = vec![1.0_f32; count];
    assert!(matches!(
        scene.interpolate_atom_property(handle, &row, &row, 1.5),
        Err(CoreError::InvalidProperty { .. })
    ));
    assert!(matches!(
        scene.interpolate_atom_property(handle, &row, &[1.0], 0.5),
        Err(CoreError::InvalidProperty { .. })
    ));
}

#[test]
fn repeated_property_interpolation_reuses_the_same_row_allocation() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let count = usize::try_from(placed.atoms.len()).map_or(0, |value| value);
    let property = AtomProperty::new(
        owner,
        "energy",
        Arc::from(vec![0.0; count]),
        AtomPropertyMeaning::Charge,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .add_atom_property(property)
        .unwrap_or_else(|error| panic!("{error}"));
    let start = vec![0.0; count];
    let end = vec![1.0; count];

    scene
        .interpolate_atom_property(handle, &start, &end, 0.25)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(first) = scene.atom_property(handle) else {
        panic!("property")
    };
    let pointer = first.values().as_ptr();
    scene
        .interpolate_atom_property(handle, &start, &end, 0.75)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(second) = scene.atom_property(handle) else {
        panic!("property")
    };
    assert_eq!(pointer, second.values().as_ptr());
}

#[test]
fn property_handles_are_stable_and_length_checked() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, placed)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let count = usize::try_from(placed.atoms.len()).map_or(0, |value| value);
    let property = AtomProperty::new(
        owner,
        "confidence",
        Arc::from(vec![0.8; count]),
        AtomPropertyMeaning::Confidence,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .add_atom_property(property)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        scene.atom_property(handle).map(AtomProperty::meaning),
        Some(AtomPropertyMeaning::Confidence)
    );
    let malformed = AtomProperty::new(
        owner,
        "short",
        Arc::from([1.0]),
        AtomPropertyMeaning::Generic,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(matches!(
        scene.add_atom_property(malformed),
        Err(CoreError::InvalidProperty { .. })
    ));
}
