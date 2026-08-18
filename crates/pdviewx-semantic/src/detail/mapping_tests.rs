use super::*;

#[test]
fn increasing_and_decreasing_mappings_round_trip() {
    for visual in [[0.0, 1.0], [1.0, 0.0]] {
        let Ok(mapping) = PropertyMapping::new([20.0, 100.0], visual) else {
            panic!("mapping is valid")
        };
        let restored = mapping.unmap(mapping.map(65.0));
        assert!((restored - 65.0).abs() < 1.0e-5);
    }
}

#[test]
fn out_of_domain_values_clamp_reversibly_to_endpoints() {
    let Ok(mapping) = PropertyMapping::new([0.0, 10.0], [0.2, 0.8]) else {
        panic!("mapping is valid")
    };
    assert!((mapping.map(-1.0) - 0.2).abs() < f32::EPSILON);
    assert!((mapping.map(11.0) - 0.8).abs() < f32::EPSILON);
}
