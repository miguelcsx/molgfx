use super::*;

#[test]
fn builders_compose_into_the_same_ir_regardless_of_grouping_order() {
    let left = Select::protein().or(Select::nucleic().and(Select::water().negate()));
    let right = Select::protein().or(Select::nucleic().and(Select::water().negate()));
    assert_eq!(left, right);
    let grouped = Select::protein()
        .or(Select::nucleic())
        .and(Select::water().negate());
    assert_ne!(left, grouped);
}

#[test]
fn geometric_and_secondary_selection_builders_validate_input() {
    assert!(Select::in_sphere(molgfx_math::Vec3::ZERO, 2.0).is_ok());
    assert!(Select::in_box(-molgfx_math::Vec3::ONE, molgfx_math::Vec3::ONE).is_ok());
    assert!(Select::in_sphere(molgfx_math::Vec3::ZERO, f32::NAN).is_err());
    assert!(Select::in_box(molgfx_math::Vec3::ONE, -molgfx_math::Vec3::ONE).is_err());
    assert_eq!(
        Select::helix(),
        Select::secondary(SecondaryStructure::Helix)
    );
}

#[test]
fn invalid_builder_input_returns_the_stable_selection_error() {
    for result in [
        Select::within(f32::NAN, Select::all()),
        Select::within(-1.0, Select::all()),
        Select::chain(" "),
    ] {
        let Err(error) = result else {
            panic!("invalid builder input must fail")
        };
        assert_eq!(error.code(), "MOLGFX-E0034");
    }
}
