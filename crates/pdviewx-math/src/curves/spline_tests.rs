use super::*;

#[test]
fn fixed_step_sampling_matches_the_gpu_kernel_contract() {
    // A collinear trace stays on the axis with unit tangents along it, and the
    // sample count follows the (steps + 1) + (segments - 1) * steps layout the
    // GPU kernel maps its linear invocation index onto.
    let points = [
        Vec3::ZERO,
        Vec3::X,
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(3.0, 0.0, 0.0),
    ];
    let mut samples = Vec::new();
    sample_catmull_rom_fixed(&points, 2, &mut samples);
    let segments = points.len() - 1;
    assert_eq!(samples.len(), (2 + 1) + (segments - 1) * 2);
    for sample in &samples {
        assert!(sample.position.y.abs() < 1e-5 && sample.position.z.abs() < 1e-5);
        assert!((sample.tangent - Vec3::X).length() < 1e-5);
    }
    // The midpoint of the interior segment 1 sits exactly halfway between its
    // evenly spaced control points, matching the shared uniform basis.
    let mid = samples
        .iter()
        .find(|s| s.segment == 1 && (s.parameter - 0.5).abs() < 1e-6);
    assert!(mid.is_some_and(|s| (s.position.x - 1.5).abs() < 1e-5));
}

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
