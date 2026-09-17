use super::*;
use crate::sample_catmull_rom;

#[test]
fn transported_frames_stay_orthonormal_and_continuous() {
    let points = [
        Vec3::ZERO,
        Vec3::X,
        Vec3::new(2.0, 1.0, 0.0),
        Vec3::new(3.0, 1.0, 1.0),
    ];
    let mut samples = Vec::new();
    sample_catmull_rom(&points, 0.01, 16, &mut samples);
    let mut frames = Vec::new();
    parallel_transport(&samples, &mut frames);
    assert_eq!(frames.len(), samples.len());
    for pair in frames.windows(2) {
        let frame = pair[1];
        assert!(frame.tangent.dot(frame.normal).abs() < 1e-5);
        assert!(frame.tangent.dot(frame.binormal).abs() < 1e-5);
        assert!(frame.normal.dot(frame.binormal).abs() < 1e-5);
        assert!(pair[0].normal.dot(pair[1].normal) >= 0.0, "frame flipped");
    }
}

#[test]
fn an_empty_trace_produces_no_frames() {
    let mut frames = Vec::new();
    parallel_transport(&[], &mut frames);
    assert!(frames.is_empty());
}
