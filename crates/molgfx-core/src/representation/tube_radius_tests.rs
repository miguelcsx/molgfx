use super::*;
use crate::{Representation, RepresentationKind};

#[test]
fn b_factor_putty_mapping_round_trips_and_clamps() {
    let mapping = match TubeRadiusMapping::b_factor([10.0, 50.0], [0.2, 0.8]) {
        Ok(mapping) => mapping,
        Err(error) => panic!("mapping builds: {error}"),
    };
    let radius = mapping.radius(30.0, 0.3);
    assert!((radius - 0.5).abs() < 1.0e-6);
    assert!(
        mapping
            .value(radius)
            .is_some_and(|value| (value - 30.0).abs() < 1.0e-5)
    );
    assert!((mapping.radius(-100.0, 0.3) - 0.2).abs() < f32::EPSILON);
    assert!((mapping.radius(100.0, 0.3) - 0.8).abs() < f32::EPSILON);
}

#[test]
fn malformed_putty_mappings_return_a_stable_typed_error() {
    let Err(error) = TubeRadiusMapping::b_factor([1.0, 1.0], [0.2, 0.8]) else {
        panic!("flat domain must fail")
    };
    assert_eq!(error.code(), "MOLGFX-E0037");
    assert!(TubeRadiusMapping::b_factor([0.0, 1.0], [0.0, 0.8]).is_err());
    assert!(TubeRadiusMapping::b_factor([0.0, 1.0], [0.8, 0.8]).is_err());
}

#[test]
fn declarative_putty_recipe_is_tube_scoped_and_validated() {
    let recipe = match Representation::putty([10.0, 50.0], [0.2, 0.8]) {
        Ok(recipe) => recipe,
        Err(error) => panic!("putty recipe validates: {error}"),
    };
    assert_eq!(recipe.kind(), RepresentationKind::Tube);

    let invalid_kind = Representation::cartoon().putty_b_factor([0.0, 1.0], [0.2, 0.8]);
    assert!(matches!(
        invalid_kind,
        Err(CoreError::InvalidProperty { .. })
    ));
    assert!(Representation::tube().tube_radius(f32::NAN).is_err());
}
