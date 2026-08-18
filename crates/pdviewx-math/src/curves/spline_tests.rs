use super::*;

#[test]
fn sampling_hits_trace_endpoints_exactly() {
    let points = [Vec3::ZERO, Vec3::X, Vec3::new(2.0, 1.0, 0.0)];
    let mut samples = Vec::new();
    sample_catmull_rom(&points, 0.01, 16, &mut samples);
    let Some(first) = samples.first() else {
        panic!("samples exist")
    };
    let Some(last) = samples.last() else {
        panic!("samples exist")
    };
    assert_eq!(first.position, points[0]);
    assert_eq!(last.position, points[2]);
}

#[test]
fn curved_segments_receive_more_samples_than_straight_ones() {
    let straight = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0, Vec3::X * 3.0];
    let curved = [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y * 2.0];
    let (mut a, mut b) = (Vec::new(), Vec::new());
    sample_catmull_rom(&straight, 0.02, 16, &mut a);
    sample_catmull_rom(&curved, 0.02, 16, &mut b);
    assert!(b.len() > a.len());
}

#[test]
fn degenerate_trace_remains_finite() {
    let mut samples = Vec::new();
    sample_catmull_rom(&[Vec3::ZERO; 4], 0.01, 8, &mut samples);
    assert!(samples.iter().all(|sample| {
        sample.position.is_finite() && sample.tangent.is_finite() && sample.tangent.is_normalized()
    }));
}
