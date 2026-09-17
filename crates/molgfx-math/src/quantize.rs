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
//! The cast lints are silenced for this module alone, because saturating
//! truncation is the operation being implemented rather than an accident of
//! one. Every cast below is preceded by a clamp onto the destination's exact
//! range and by a non-finite guard, so the two failure modes the lints warn
//! about — wrapping and an undefined non-finite result — are unreachable. The
//! sibling tests check that against a linear scan of every representable
//! output. Do not push these casts back behind a bounded search: this code runs
//! once per atom and the search cost eight to ten iterations to compute what a
//! multiply and a cast compute exactly.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

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
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
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
    (value.clamp(0.0, 255.0) + 0.5) as u8
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
    value.clamp(0.0, f32::from(u16::MAX)) as u16
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
    (value.clamp(0.0, 1.0) * levels as f32) as u32
}

#[cfg(test)]
#[path = "quantize_tests.rs"]
mod tests;
