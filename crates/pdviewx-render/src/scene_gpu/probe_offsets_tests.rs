use super::*;

#[test]
fn every_offset_has_the_probe_radius_length() {
    let probe = 1.4_f32;
    for offset in probe_offsets(probe) {
        let length = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]).sqrt();
        assert!(
            (length - probe).abs() < 1.0e-4,
            "offset length {length} should equal the probe radius {probe}"
        );
        assert!(
            offset[3].abs() < 1.0e-6,
            "the fourth lane is unused padding"
        );
    }
}

#[test]
fn the_directions_average_to_near_zero_so_the_roll_is_isotropic() {
    let sum = probe_offsets(1.0).iter().fold([0.0_f32; 3], |acc, offset| {
        [acc[0] + offset[0], acc[1] + offset[1], acc[2] + offset[2]]
    });
    let mean = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt()
        / f32::from(PROBE_SAMPLE_COUNT);
    assert!(
        mean < 0.1,
        "a Fibonacci sphere has no directional bias: {mean}"
    );
}

#[test]
fn equal_area_band_midpoints_do_not_waste_samples_on_the_poles() {
    let offsets = probe_offsets(1.0);
    assert!(offsets.iter().all(|offset| offset[1].abs() < 1.0));
    let y_sum = offsets.iter().map(|offset| offset[1]).sum::<f32>();
    assert!(y_sum.abs() < 1.0e-6, "latitude bands balance exactly");
}

#[test]
fn generation_is_deterministic() {
    assert_eq!(probe_offsets(1.4), probe_offsets(1.4));
}
