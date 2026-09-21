//! Deterministic float-to-integer quantisation.
//!
//! Crossing the float/integer boundary needs one house form, because the two
//! plausible conversions disagree: truncation drops the fractional part, and
//! round-to-nearest moves a value up to half a step. Both are stated here
//! explicitly so a caller picks the one its format documents rather than
//! inheriting whatever `as` happens to do.
//!
//! Every function is branch-free apart from its non-finite guard and costs a
//! multiply and a cast. They run per atom, per voxel and per primitive, so the
//! cost of the conversion is the cost of the arithmetic in it.
//!
//! Every conversion is preceded by a clamp onto the destination's exact range
//! and by a non-finite guard. The sibling tests check the boundary behavior.

use num_traits::ToPrimitive as _;

/// Rounds a unit value to unorm8 with ties resolved upward.
///
/// `value` is clamped to `[0, 1]` first, so the result covers the full `0..=255`
/// range with 255 meaning exactly one. A non-finite input quantises to zero.
#[inline]
#[must_use]
pub fn unorm8(value: f32) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    // The clamp bounds the sum below 256.0, and `as` saturates rather than
    // wrapping, so the cast cannot alias a different byte.
    (value.clamp(0.0, 1.0) * 255.0 + 0.5)
        .to_u8()
        .into_iter()
        .fold(0, |_, converted| converted)
}

/// Rounds a value already expressed in `0..=255` units, ties upward.
///
/// This differs from [`unorm8`] only in the domain it accepts: the caller has
/// already scaled, so no second multiply is applied. A non-finite input
/// quantises to zero.
#[inline]
#[must_use]
pub fn round_u8(value: f32) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 255.0) + 0.5)
        .to_u8()
        .into_iter()
        .fold(0, |_, converted| converted)
}

/// Truncates a non-negative value to `u16`, saturating at the maximum.
///
/// Truncation rather than rounding, because the callers encode a fixed-point
/// magnitude where rounding up could report an extent the source never had.
#[inline]
#[must_use]
pub fn truncate_u16(value: f32) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    value
        .clamp(0.0, f32::from(u16::MAX))
        .to_u16()
        .into_iter()
        .fold(0, |_, converted| converted)
}

/// Truncates a unit value onto a grid of `levels + 1` steps.
///
/// Used for spatial cell indices, where the grid is a partition and a value on
/// a cell boundary belongs to the cell above it. A non-finite input lands in
/// cell zero.
#[inline]
#[must_use]
pub fn unit_to_grid(value: f32, levels: u32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    // `as` saturates at `levels` for an input of exactly one, so the returned
    // index is always a valid cell.
    let levels = levels
        .to_f32()
        .into_iter()
        .fold(f32::MAX, |_, converted| converted);
    (value.clamp(0.0, 1.0) * levels)
        .to_u32()
        .into_iter()
        .fold(0, |_, converted| converted)
}

#[cfg(test)]
#[path = "quantize_tests.rs"]
mod tests;
