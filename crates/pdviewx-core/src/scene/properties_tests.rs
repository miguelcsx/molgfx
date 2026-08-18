use crate::{AtomProperty, AtomPropertyMeaning, CoreError, ScalarFieldSemantics, Scene};
use std::sync::Arc;

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
