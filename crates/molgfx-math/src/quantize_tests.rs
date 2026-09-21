//! The reference for these is the bounded search this module replaced: a
//! linear scan over every representable output, selecting the same step the
//! search would have settled on. Agreeing with it across the whole domain is
//! what makes the arithmetic form a drop-in.

use super::{round_u8, truncate_u16, unit_to_grid, unorm8};
use num_traits::ToPrimitive as _;

/// The bounded search the arithmetic replaced: the largest step at or below
/// the target, then the nearer of that step and its successor, ties upward.
fn searched_round_u8(target: f32) -> u8 {
    let mut low = 0u16;
    let mut high = u16::from(u8::MAX);
    while low < high {
        let middle = (low + high).div_ceil(2);
        if f32::from(middle) <= target {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let upper = (low + 1).min(u16::from(u8::MAX));
    let selected = if target - f32::from(low) < f32::from(upper) - target {
        low
    } else {
        upper
    };
    u8::try_from(selected).expect("bounded byte search stays in range")
}

#[test]
fn rounding_a_unit_value_agrees_with_the_bounded_search_at_every_step() {
    for step in 0..=2550u32 {
        let value = step.to_f32().expect("fixture step fits f32") / 2550.0;
        assert_eq!(
            unorm8(value),
            searched_round_u8(value.clamp(0.0, 1.0) * 255.0),
            "unit value {value}"
        );
    }
}

#[test]
fn rounding_a_scaled_value_agrees_with_the_bounded_search_at_every_step() {
    for step in 0..=5100u32 {
        let value = step.to_f32().expect("fixture step fits f32") / 20.0;
        assert_eq!(round_u8(value), searched_round_u8(value), "value {value}");
    }
}

#[test]
fn a_unit_value_of_one_quantises_to_the_full_byte() {
    assert_eq!(unorm8(1.0), 255);
    assert_eq!(unorm8(0.0), 0);
}

#[test]
fn a_half_step_rounds_upward_so_ties_never_bias_downward() {
    // 0.5/255 sits exactly between byte 0 and byte 1.
    assert_eq!(unorm8(0.5 / 255.0), 1);
    assert_eq!(round_u8(0.5), 1);
    assert_eq!(round_u8(1.5), 2);
}

#[test]
fn values_outside_the_domain_clamp_rather_than_wrapping() {
    assert_eq!(unorm8(-4.0), 0);
    assert_eq!(unorm8(9.0), 255);
    assert_eq!(round_u8(-1.0), 0);
    assert_eq!(round_u8(1.0e9), 255);
    assert_eq!(truncate_u16(-1.0), 0);
    assert_eq!(truncate_u16(1.0e9), u16::MAX);
}

#[test]
fn a_non_finite_input_quantises_to_zero_rather_than_a_neighbouring_step() {
    assert_eq!(unorm8(f32::NAN), 0);
    assert_eq!(unorm8(f32::INFINITY), 0);
    assert_eq!(round_u8(f32::NAN), 0);
    assert_eq!(truncate_u16(f32::NAN), 0);
    assert_eq!(unit_to_grid(f32::NAN, 1023), 0);
}

#[test]
fn a_fixed_point_magnitude_truncates_instead_of_rounding_up() {
    // 1.999 hundredths must not report two hundredths of extent.
    assert_eq!(truncate_u16(1.999), 1);
    assert_eq!(truncate_u16(2.0), 2);
}

#[test]
fn a_grid_index_partitions_the_unit_interval_and_never_leaves_the_grid() {
    assert_eq!(unit_to_grid(0.0, 1023), 0);
    assert_eq!(unit_to_grid(1.0, 1023), 1023);
    for step in 0..=4096u32 {
        let value = step.to_f32().expect("fixture step fits f32") / 4096.0;
        assert!(unit_to_grid(value, 1023) <= 1023, "value {value}");
    }
}

#[test]
fn a_grid_index_agrees_with_the_bounded_search_it_replaced() {
    let searched = |value: f32| {
        let target = value * 1023.0;
        let mut low = 0u16;
        let mut high = 1023u16;
        while low < high {
            let middle = (low + high).div_ceil(2);
            if f32::from(middle) <= target {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        u32::from(low)
    };
    for step in 0..=8192u32 {
        let value = step.to_f32().expect("fixture step fits f32") / 8192.0;
        assert_eq!(unit_to_grid(value, 1023), searched(value), "value {value}");
    }
}
