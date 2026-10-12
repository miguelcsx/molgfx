use super::*;

#[test]
fn discrete_boundaries_use_adjacent_residue_midpoints_and_extrapolated_ends() {
    let points = [
        Vec3::ZERO,
        Vec3::new(2.0, 1.0, 0.0),
        Vec3::X * 4.0,
        Vec3::new(6.0, 1.0, 0.0),
    ];
    let output = frames(&points);
    assert_eq!(output.len(), points.len() + 1);
    assert_eq!(output[0].center, Vec3::new(-1.0, 0.5, 0.0));
    assert_eq!(output[4].center, Vec3::new(7.0, 0.5, 0.0));
    for index in 1..4 {
        assert_eq!(
            output[index].center,
            (points[index - 1] + points[index]) * 0.5
        );
    }
    for pair in output.windows(2) {
        assert!(pair[0].height.dot(pair[1].height) >= 0.0);
    }
    assert_eq!(output[4].width, output[3].width);
    assert_eq!(output[4].height, output[3].height);
}

#[test]
fn coincident_and_collinear_guides_keep_finite_deterministic_unit_frames() {
    for points in [
        vec![Vec3::ZERO; 4],
        vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0, Vec3::X * 3.0],
    ] {
        let first = frames(&points);
        let repeated = frames(&points);
        for (a, b) in first.iter().zip(repeated) {
            assert!(a.center.is_finite());
            assert_eq!(a.width, b.width);
            assert_eq!(a.height, b.height);
            assert!((a.width.length() - 1.0).abs() < 1e-6);
            assert!((a.height.length() - 1.0).abs() < 1e-6);
            assert!(a.width.dot(a.height).abs() < 1e-6);
        }
    }
}

#[test]
fn twisted_interior_cells_curve_without_changing_their_exact_boundary_frames() {
    let frames = [
        StrandFrame {
            center: Vec3::new(-2.0, 2.0, 0.0),
            width: Vec3::Z,
            height: Vec3::Y,
        },
        StrandFrame {
            center: Vec3::ZERO,
            width: Vec3::Z,
            height: Vec3::Y,
        },
        StrandFrame {
            center: Vec3::X * 2.0,
            width: Vec3::Y,
            height: Vec3::Z,
        },
        StrandFrame {
            center: Vec3::new(4.0, 2.0, 0.0),
            width: Vec3::Y,
            height: Vec3::Z,
        },
    ];
    assert_eq!(sample(&frames, 1, 0.0).center, frames[1].center);
    assert_eq!(sample(&frames, 1, 1.0).center, frames[2].center);
    assert_eq!(sample(&frames, 1, 0.5).center, Vec3::new(1.0, -0.25, 0.0));
    let midpoint = sample(&frames, 1, 0.5);
    assert!((midpoint.width.length() - 1.0).abs() < 1e-6);
    assert!((midpoint.height.length() - 1.0).abs() < 1e-6);
    assert_eq!(
        sample(&frames, 0, 0.5).center,
        frames[0].center.lerp(frames[1].center, 0.5)
    );
}
