use super::*;

#[test]
fn calibrated_fields_require_quantity_units_and_provenance() {
    let valid = ScalarFieldSemantics::quantity(
        Arc::from("electrostatic potential"),
        Arc::from("kT/e"),
        Arc::from("pdbiox:apbs-run-42"),
    );
    assert!(valid.is_ok());
    let invalid = ScalarFieldSemantics::quantity(
        Arc::from("electrostatic potential"),
        Arc::from(" "),
        Arc::from("pdbiox:apbs-run-42"),
    );
    assert!(invalid.is_err());
}

#[test]
fn scalar_ramps_expose_the_exact_reversible_legend_stops() {
    let values = [-5.0, 0.0, 5.0];
    let colors = [
        Rgba8::opaque(0, 0, 255),
        Rgba8::WHITE,
        Rgba8::opaque(255, 0, 0),
    ];
    let ramp = match ScalarRamp::new(values, colors) {
        Ok(ramp) => ramp,
        Err(error) => panic!("ramp validates: {error}"),
    };
    for (actual, expected) in ramp.values().into_iter().zip(values) {
        assert!((actual - expected).abs() < f32::EPSILON);
    }
    assert_eq!(ramp.colors(), colors);
}

#[test]
fn sequential_ramps_sample_endpoints_midpoint_and_missing_values() {
    let missing = Rgba8::opaque(112, 112, 112);
    let ramp = ScalarRamp::sequential([10.0, 30.0]);
    let colors = ramp.colors();
    assert_eq!(ramp.sample(-1.0, missing), colors[0]);
    assert_eq!(ramp.sample(20.0, missing), colors[1]);
    assert_eq!(ramp.sample(300.0, missing), colors[2]);
    assert_eq!(ramp.sample(f32::NAN, missing), missing);
}

#[test]
fn contour_intervals_cannot_collapse_to_a_feature_toggle() {
    assert!(ScalarContours::new(0.5, 1.25).is_ok());
    assert!(ScalarContours::new(0.0, 1.25).is_err());
    assert!(ScalarContours::new(0.5, f32::NAN).is_err());
}
