use super::*;

#[test]
fn calibrated_fields_require_quantity_units_and_provenance() {
    let valid = ScalarFieldSemantics::quantity(
        Arc::from("electrostatic potential"),
        Arc::from("kT/e"),
        Arc::from("molframe:apbs-run-42"),
    );
    assert!(valid.is_ok());
    let invalid = ScalarFieldSemantics::quantity(
        Arc::from("electrostatic potential"),
        Arc::from(" "),
        Arc::from("molframe:apbs-run-42"),
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
    let ramp = match ScalarRamp::new(&values, &colors) {
        Ok(ramp) => ramp,
        Err(error) => panic!("ramp validates: {error}"),
    };
    for (actual, expected) in ramp.values().iter().zip(values) {
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

fn stops(count: usize) -> Vec<Rgba8> {
    (0..count)
        .map(|index| {
            let level = u8::try_from(index * 8).unwrap_or(u8::MAX);
            Rgba8::opaque(level, 255 - level, 0)
        })
        .collect()
}

#[test]
fn ramps_hold_up_to_sixteen_stops_and_refuse_more_or_fewer() {
    let values = |count: usize| -> Vec<f32> {
        (0..count)
            .map(|index| u16::try_from(index).map_or(0.0, f32::from))
            .collect()
    };
    assert!(ScalarRamp::new(&values(16), &stops(16)).is_ok());
    assert!(ScalarRamp::new(&values(17), &stops(17)).is_err());
    assert!(ScalarRamp::new(&values(1), &stops(1)).is_err());
    assert!(ScalarRamp::new(&values(4), &stops(3)).is_err());
    assert!(ScalarRamp::new(&[0.0, 0.0], &stops(2)).is_err());
    assert!(ScalarRamp::new(&[0.0, f32::NAN], &stops(2)).is_err());
}

#[test]
fn a_many_stop_ramp_interpolates_inside_the_segment_that_holds_the_value() {
    let colors = stops(5);
    let Ok(ramp) = ScalarRamp::evenly([0.0, 4.0], &colors) else {
        panic!("an even ramp builds")
    };
    let missing = Rgba8::opaque(0, 0, 0);
    for (index, color) in colors.iter().enumerate() {
        let value = u8::try_from(index).map_or(0.0, f32::from);
        assert_eq!(ramp.sample(value, missing), *color, "stop {index}");
    }
    let halfway = ramp.sample(2.5, missing);
    assert_eq!(halfway.r, 20);
    assert_eq!(ramp.sample(-3.0, missing), colors[0]);
    assert_eq!(ramp.sample(99.0, missing), colors[4]);
    let [low, high] = ramp.domain();
    assert!((low - 0.0).abs() < f32::EPSILON && (high - 4.0).abs() < f32::EPSILON);
}
