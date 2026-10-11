use super::*;

#[test]
fn peptide_widths_align_opposite_oxygens_and_keep_transport_for_missing_planes() {
    assert_eq!(
        projected_blend(Vec3::Y, -Vec3::Y, 0.5, Vec3::X),
        Some(Vec3::Y)
    );
    assert_eq!(projected_blend(Vec3::X, Vec3::Y, 0.5, Vec3::X), None);
    assert_eq!(projected_blend(Vec3::ZERO, Vec3::Y, 0.5, Vec3::X), None);
    let width = projected_blend(Vec3::Y, Vec3::Z, 0.5, Vec3::X).expect("plane interpolates");
    assert!(width.dot(Vec3::X).abs() < 1e-6);
    assert!((width.length() - 1.0).abs() < 1e-6);
    let samples = [CurveSample {
        position: Vec3::ZERO,
        tangent: Vec3::X,
        segment: 0,
        parameter: 0.5,
    }];
    let mut frames = [TransportFrame {
        tangent: Vec3::X,
        normal: Vec3::Z,
        binormal: -Vec3::Y,
    }];
    let initial = frames;
    orient_to_peptides(&samples, &[Vec3::ZERO; 2], &mut frames);
    assert_eq!(frames, initial);
}
