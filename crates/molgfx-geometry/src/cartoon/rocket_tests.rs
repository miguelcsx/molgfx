use super::*;

#[test]
fn fitted_helix_columns_use_residue_order_instead_of_projecting_each_atom() {
    let points = [Vec3::ZERO, Vec3::new(1.0, 8.0, 0.0), Vec3::X * 2.0];
    let mut fitted = Vec::new();
    project_trace(&points, &[SecondaryStructure::AlphaHelix; 3], &mut fitted);
    for (point, expected_x) in fitted.iter().zip([-0.5, 1.0, 2.5]) {
        assert!((point.x - expected_x).abs() < 1e-6);
        assert!((point.y - 8.0 / 3.0).abs() < 1e-6);
    }
}

#[test]
fn ribbon_strands_filter_neighbors_but_leave_isolated_bridges_and_coils_unchanged() {
    let points = [
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(2.0, -1.0, 0.0),
        Vec3::new(3.0, 1.0, 0.0),
    ];
    let styles = [
        SecondaryStructure::Strand,
        SecondaryStructure::Strand,
        SecondaryStructure::BetaBridge,
        SecondaryStructure::Coil,
    ];
    let mut fitted = Vec::new();
    filter_strands(&points, &styles, &mut fitted);
    assert_eq!(fitted[0], Vec3::new(0.25, -0.5, 0.0));
    assert_eq!(fitted[1], Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(&fitted[2..], &points[2..]);
}
#[test]
fn two_or_more_helix_guides_fit_a_column_with_half_residue_terminal_extensions() {
    let points = [Vec3::ZERO, Vec3::ONE, Vec3::new(2.0, 0.0, 2.0)];
    let styles = [SecondaryStructure::AlphaHelix; 3];
    let mut fitted = Vec::new();
    project_trace(&points, &styles, &mut fitted);
    assert!(
        (fitted[1] - fitted[0])
            .cross(fitted[2] - fitted[0])
            .length()
            < 1e-5
    );
    assert!(fitted[1].distance(points[1]) > 0.1);
    assert_eq!(helix_run(&styles, 1), Some(0..3));
    assert_eq!(helix_run(&styles[..2], 0), Some(0..2));
    project_trace(&points[..2], &styles[..2], &mut fitted);
    assert_eq!(
        fitted,
        [points[0] - Vec3::ONE * 0.5, points[1] + Vec3::ONE * 0.5]
    );
}

#[test]
fn classic_short_beta_runs_stay_at_their_original_coordinates_and_bridges_extend_complete_sheets() {
    let points = [
        Vec3::ZERO,
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::X * 2.0,
        Vec3::X * 3.0,
    ];
    let mut fitted = Vec::new();
    let short = [
        SecondaryStructure::Strand,
        SecondaryStructure::BetaBridge,
        SecondaryStructure::Coil,
        SecondaryStructure::Coil,
    ];
    project_trace(&points, &short, &mut fitted);
    assert_eq!(fitted, points);
    let sheet = [
        SecondaryStructure::Strand,
        SecondaryStructure::BetaBridge,
        SecondaryStructure::Strand,
        SecondaryStructure::Coil,
    ];
    project_trace(&points, &sheet, &mut fitted);
    assert_eq!(fitted[0], points[0] + points[1] * 0.5 - points[2] * 0.5);
    assert_eq!(fitted[2], points[2] + points[1] * 0.5 - points[0] * 0.5);
    assert_eq!(fitted[3], points[3]);
}
