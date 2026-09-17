use super::*;

#[test]
fn strings_and_builders_lower_to_the_same_selection_ir() {
    let parsed = match "polymer and not water".parse::<Select>() {
        Ok(parsed) => parsed,
        Err(error) => panic!("selection parses: {error}"),
    };
    assert_eq!(parsed, Select::polymer().and(Select::water().negate()));
}

#[test]
fn boolean_precedence_and_parentheses_are_deterministic() {
    let implicit = match "protein or nucleic and not water".parse::<Select>() {
        Ok(parsed) => parsed,
        Err(error) => panic!("selection parses: {error}"),
    };
    assert_eq!(
        implicit,
        Select::protein().or(Select::nucleic().and(Select::water().negate()))
    );
    let grouped = match "(protein or nucleic) and not water".parse::<Select>() {
        Ok(parsed) => parsed,
        Err(error) => panic!("selection parses: {error}"),
    };
    assert_ne!(implicit, grouped);
}

#[test]
fn spatial_strings_use_the_validated_builder() {
    let parsed = match "within 5.5 of (ligand or water)".parse::<Select>() {
        Ok(parsed) => parsed,
        Err(error) => panic!("selection parses: {error}"),
    };
    let expected = match Select::within(5.5, Select::ligands().or(Select::water())) {
        Ok(expected) => expected,
        Err(error) => panic!("typed selection builds: {error}"),
    };
    assert_eq!(parsed, expected);
}

#[test]
fn hierarchy_and_property_strings_use_the_same_typed_builders() {
    let parsed = match "chain A and resname GLY and element C and b_factor >= 10".parse::<Select>()
    {
        Ok(parsed) => parsed,
        Err(error) => panic!("rich selection parses: {error}"),
    };
    let b_factor = match Select::b_factor(PropertyComparison::GreaterOrEqual, 10.0) {
        Ok(value) => value,
        Err(error) => panic!("B-factor predicate builds: {error}"),
    };
    let expected = match Select::chain("A") {
        Ok(value) => value,
        Err(error) => panic!("chain predicate builds: {error}"),
    }
    .and(match Select::residue_name("GLY") {
        Ok(value) => value,
        Err(error) => panic!("residue predicate builds: {error}"),
    })
    .and(match Select::element("C") {
        Ok(value) => value,
        Err(error) => panic!("element predicate builds: {error}"),
    })
    .and(b_factor);
    assert_eq!(parsed, expected);
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
    assert_eq!(
        Select::hydrogen(),
        Select::from_str("hydrogen").ok().unwrap()
    );
}

#[test]
fn malformed_strings_return_the_stable_selection_error() {
    for source in [
        "",
        "within nope of ligand",
        "protein and",
        "(water",
        "magic",
    ] {
        let Err(error) = source.parse::<Select>() else {
            panic!("malformed selection must fail: {source}")
        };
        assert_eq!(error.code(), "MOLGFX-E0034");
    }
    assert!(Select::within(f32::NAN, Select::all()).is_err());
    assert!(Select::within(-1.0, Select::all()).is_err());
}
