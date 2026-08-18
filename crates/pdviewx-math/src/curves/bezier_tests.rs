use super::*;

#[test]
fn cubic_bezier_sampling_keeps_endpoints_and_count() {
    let control = [Vec3::ZERO, Vec3::Y, Vec3::ONE, Vec3::X];
    let mut points = Vec::new();
    sample_cubic_bezier(control, 8, &mut points);
    assert_eq!(points.len(), 9);
    assert_eq!(points.first(), Some(&control[0]));
    assert_eq!(points.last(), Some(&control[3]));
}

#[test]
fn zero_segments_still_emits_one_line_segment() {
    let control = [Vec3::ZERO, Vec3::ZERO, Vec3::X, Vec3::X];
    let mut points = Vec::new();
    sample_cubic_bezier(control, 0, &mut points);
    assert_eq!(points, vec![Vec3::ZERO, Vec3::X]);
}
