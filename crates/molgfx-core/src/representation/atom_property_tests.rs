use super::*;
use crate::handle::RawHandle;

fn owner() -> StructureHandle {
    StructureHandle(RawHandle::new_for_test(0, 0))
}

fn property_handle() -> AtomPropertyHandle {
    AtomPropertyHandle(RawHandle::new_for_test(1, 0))
}

#[test]
fn property_retains_shared_values_and_ignores_missing_values_in_its_domain() {
    let values: Arc<[f32]> = Arc::from([f32::NAN, 0.2, 0.8]);
    let property = AtomProperty::new(
        owner(),
        "confidence",
        Arc::clone(&values),
        AtomPropertyMeaning::Confidence,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(Arc::ptr_eq(&values, property.shared_values()));
    for (actual, expected) in property.finite_domain().into_iter().zip([0.2, 0.8]) {
        assert!((actual - expected).abs() < f32::EPSILON);
    }
}

#[test]
fn invalid_property_values_are_typed_errors() {
    let result = AtomProperty::new(
        owner(),
        "bad",
        Arc::from([f32::INFINITY]),
        AtomPropertyMeaning::Generic,
        ScalarFieldSemantics::UncalibratedRank,
    );
    assert!(matches!(result, Err(CoreError::InvalidProperty { .. })));
}

#[test]
fn confidence_appearance_is_monotonic_reversible_and_marks_translucency() {
    let mapping = PropertyAppearance::confidence(property_handle(), [20.0, 100.0])
        .unwrap_or_else(|error| panic!("{error}"));
    let low = mapping.sample(20.0);
    let high = mapping.sample(100.0);
    assert!(low.opacity < high.opacity);
    assert!(low.softness_pixels > high.softness_pixels);
    assert!(mapping.is_translucent());
    let restored = mapping
        .value_from_softness(mapping.sample(63.0).softness_pixels)
        .unwrap_or_else(|| panic!("softness channel varies"));
    assert!((restored - 63.0).abs() < 1.0e-4);
}

#[test]
fn appearance_rejects_flat_or_out_of_range_encodings() {
    let invalid = PropertyAppearance::new(
        property_handle(),
        [0.0, 1.0],
        [1.0, 1.0],
        [0.0, 0.0],
        PropertyAppearanceSample {
            opacity: 1.0,
            softness_pixels: 0.0,
        },
    );
    assert!(matches!(invalid, Err(CoreError::InvalidProperty { .. })));
}
