use super::*;

#[test]
fn puckering_colour_changes_continuously_between_amplitude_stops() {
    let first = pucker_color(0.20, 77);
    let next = pucker_color(0.21, 77);
    assert_eq!(first.a, 77);
    assert_eq!(next.a, 77);
    assert!(first.r.abs_diff(next.r) < 10);
    assert!(first.g.abs_diff(next.g) < 10);
    assert!(first.b.abs_diff(next.b) < 10);
}

#[test]
fn gpu_palette_uses_the_same_amplitudes_and_colours_as_the_cpu() {
    let mut values = Vec::new();
    append_palette(&mut values, 123);
    for ((amplitude, [r, g, b]), pair) in PUCKER_STOPS.iter().zip(values.as_chunks::<2>().0) {
        assert_eq!(pair[0].to_bits(), (f32::from(*amplitude) / 100.0).to_bits());
        assert_eq!(pair[1].to_bits().to_le_bytes(), [*r, *g, *b, 123]);
    }
}

#[test]
fn the_mean_plane_removes_tilt_and_retains_chair_puckering() {
    let mut points = [Vec3::ZERO; 6];
    for (index, point) in points.iter_mut().enumerate() {
        let angle = std::f32::consts::TAU * f32::from(u8::try_from(index).unwrap()) / 6.0;
        *point = Vec3::new(angle.cos(), angle.sin(), angle.cos() * 0.7);
    }
    assert!(pucker_amplitude(&points) < 1e-6);
    for (index, point) in points.iter_mut().enumerate() {
        point.z += if index % 2 == 0 { 0.3 } else { -0.3 };
    }
    let amplitude = pucker_amplitude(&points);
    assert!(amplitude > 0.5);
    for point in &mut points {
        *point += Vec3::new(100.0, -7.0, 30.0);
    }
    assert!((pucker_amplitude(&points) - amplitude).abs() < 1e-4);
}
