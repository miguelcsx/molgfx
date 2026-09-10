//! Each kernel is checked against the scalar form it replaces, over lengths
//! that straddle the lane width so the remainder path is never skipped.
//!
//! Comparisons are exact on purpose: these kernels only compare, take minima
//! and take maxima, so agreeing with the scalar form to the bit is the claim
//! being tested, not an approximation of it. The index-to-float casts build
//! fixtures from loop counters bounded by literals in this file.
#![allow(clippy::cast_precision_loss)]

use super::{all_finite, all_finite_non_negative, all_within, min_max, points_aabb, spheres_aabb};
use crate::{Aabb, Vec3};

fn column(len: usize) -> Vec<f32> {
    (0..len).map(|index| index as f32 * 0.5 - 3.0).collect()
}

fn points(len: usize) -> Vec<[f32; 3]> {
    (0..len)
        .map(|index| {
            let value = index as f32;
            [value * 0.5 - 3.0, 2.0 - value, value * 0.25]
        })
        .collect()
}

#[test]
fn finiteness_agrees_with_the_scalar_form_across_the_lane_boundary() {
    for len in 0..40usize {
        let values = column(len);
        assert_eq!(
            all_finite(&values),
            values.iter().all(|value| value.is_finite()),
            "length {len}"
        );
    }
}

#[test]
fn a_single_non_finite_value_is_found_wherever_it_sits_in_the_block() {
    for len in 1..40usize {
        for position in 0..len {
            for poison in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut values = column(len);
                values[position] = poison;
                assert!(!all_finite(&values), "length {len} position {position}");
            }
        }
    }
}

#[test]
fn a_negative_radius_fails_the_non_negative_sweep() {
    for len in 1..24usize {
        for position in 0..len {
            let mut values = vec![1.0f32; len];
            values[position] = -0.5;
            assert!(!all_finite_non_negative(&values));
            values[position] = f32::NAN;
            assert!(!all_finite_non_negative(&values));
        }
        assert!(all_finite_non_negative(&vec![0.0f32; len]));
    }
}

#[test]
fn a_range_sweep_agrees_with_the_scalar_form() {
    for len in 0..24usize {
        let values = column(len);
        assert_eq!(
            all_within(&values, -1.0, 4.0),
            values.iter().all(|value| *value >= -1.0 && *value <= 4.0),
            "length {len}"
        );
    }
}

#[test]
fn extrema_agree_with_the_scalar_form_across_the_lane_boundary() {
    for len in 1..40usize {
        let values = column(len);
        let expected = values
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |acc, v| {
                (acc.0.min(*v), acc.1.max(*v))
            });
        assert_eq!(min_max(&values), expected, "length {len}");
    }
}

#[test]
fn extrema_skip_non_finite_rows_rather_than_reporting_them() {
    let mut values = column(20);
    values[3] = f32::NAN;
    let (low, high) = min_max(&values);
    assert!(low.is_finite() && high.is_finite());
    assert_eq!(low.to_bits(), (-3.0f32).to_bits());
}

#[test]
fn an_empty_or_wholly_non_finite_column_reports_the_empty_range() {
    assert_eq!(min_max(&[]), (f32::INFINITY, f32::NEG_INFINITY));
    assert_eq!(
        min_max(&[f32::NAN, f32::NAN, f32::NAN]),
        (f32::INFINITY, f32::NEG_INFINITY)
    );
}

#[test]
fn a_point_bound_agrees_with_the_scalar_builder_across_the_lane_boundary() {
    for len in 0..40usize {
        let column = points(len);
        let expected = Aabb::from_points(column.iter().map(|p| Vec3::from_array(*p)));
        assert_eq!(points_aabb(&column), expected, "length {len}");
    }
}

#[test]
fn a_point_bound_skips_a_non_finite_row() {
    let mut column = points(20);
    column[7] = [f32::NAN, 1.0, 2.0];
    let expected = Aabb::from_points(column.iter().map(|p| Vec3::from_array(*p)));
    assert_eq!(points_aabb(&column), expected);
}

#[test]
fn a_sphere_bound_agrees_with_the_scalar_builder_across_the_lane_boundary() {
    for len in 0..40usize {
        let centers = points(len);
        let radii: Vec<f32> = (0..len).map(|index| 1.0 + index as f32 * 0.1).collect();
        let mut expected = Aabb::EMPTY;
        for (center, radius) in centers.iter().zip(&radii) {
            expected.extend_sphere(Vec3::from_array(*center), *radius);
        }
        assert_eq!(spheres_aabb(&centers, &radii), expected, "length {len}");
    }
}

#[test]
fn a_sphere_bound_uses_the_radius_magnitude_so_a_negative_radius_still_encloses() {
    let centers = [[0.0f32, 0.0, 0.0]];
    assert_eq!(
        spheres_aabb(&centers, &[-2.0]),
        spheres_aabb(&centers, &[2.0])
    );
}
