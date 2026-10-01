use super::*;

#[test]
fn triclinic_axes_reproduce_lengths_angles_and_translated_fractional_corners() {
    let lengths = [10.0, 11.0, 12.0];
    let angles = [80.0, 90.0, 100.0];
    let origin = Vec3::new(3.0, -4.0, 5.0);
    let cell = CrystalCell::new(lengths, angles)
        .unwrap()
        .with_origin(origin)
        .unwrap();
    let [a, b, c] = [
        cell.fractional_to_cartesian([1.0, 0.0, 0.0]) - origin,
        cell.fractional_to_cartesian([0.0, 1.0, 0.0]) - origin,
        cell.fractional_to_cartesian([0.0, 0.0, 1.0]) - origin,
    ];
    for (axis, length) in [a, b, c].into_iter().zip(lengths) {
        assert!((axis.length() - length).abs() < 1.0e-5);
    }
    for ((first, second), angle) in [(b, c), (a, c), (a, b)].into_iter().zip(angles) {
        let cosine = first.dot(second) / (first.length() * second.length());
        assert!((cosine.acos().to_degrees() - angle).abs() < 1.0e-4);
    }
    assert!(a.cross(b).dot(c) > 0.0);
    assert!((cell.corners()[7] - (origin + a + b + c)).length() < 1.0e-5);
    for (start, end) in cell.edges() {
        assert!(cell.corners().contains(&start));
        assert!(cell.corners().contains(&end));
        let delta = end - start;
        assert!(
            [a, b, c]
                .iter()
                .any(|axis| (delta - *axis).length() < 1.0e-5)
        );
    }
}

#[test]
fn a_cell_rejects_nonfinite_or_collapsed_translated_boundaries() {
    let cell = CrystalCell::new([1.0; 3], [90.0; 3]).unwrap();
    for origin in [
        Vec3::splat(f32::NAN),
        Vec3::splat(f32::INFINITY),
        Vec3::splat(f32::MAX),
    ] {
        assert!(matches!(
            cell.with_origin(origin),
            Err(CoreError::InvalidPrimitive { .. })
        ));
    }
    assert_eq!(cell.origin(), Vec3::ZERO);
}

#[test]
fn impossible_and_degenerate_cells_have_typed_errors() {
    for (lengths, angles) in [
        ([1.0; 3], [10.0, 10.0, 170.0]),
        ([1.0; 3], [60.0, 60.0, 120.0]),
        ([1.0e-8; 3], [90.0; 3]),
        ([f32::NAN; 3], [90.0; 3]),
        ([1.0; 3], [f32::INFINITY; 3]),
    ] {
        assert!(matches!(
            CrystalCell::new(lengths, angles),
            Err(CoreError::InvalidPrimitive { .. })
        ));
    }
}
