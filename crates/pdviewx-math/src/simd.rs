//! Width-agnostic kernels over the columns the load path sweeps.
//!
//! These are written as fixed-width chunk loops rather than intrinsics. LLVM
//! vectorises this shape reliably on every target the engine builds for, the
//! code stays readable, and there is no runtime dispatch or unchecked code — the
//! crate keeps `forbid(unsafe_code)`. Portable SIMD would express it more
//! directly but is not on stable, and an intrinsic version would be three
//! implementations to keep in agreement.
//!
//! Every kernel is exact and order-independent: they compare, they take minima
//! and maxima, and they OR bit patterns together. None of them sums, so none of
//! them can give a different answer for a different lane count. That is what
//! makes them safe under the determinism contract.
//!
//! Each is `O(n)` with a `LANES`-wide body and a scalar remainder.

use crate::{Aabb, Vec3};

/// Elements processed per loop body.
const LANES: usize = 8;

/// Exponent field of an IEEE-754 binary32. All ones means infinity or NaN.
const EXPONENT_MASK: u32 = 0x7f80_0000;

/// Whether every value is finite.
///
/// Tests the exponent field rather than calling `is_finite` per element: the
/// bit test has no branch, so the loop stays one straight line and vectorises.
/// Accumulating with `&` over "is finite" lets the whole block reduce to a
/// single comparison at the end.
#[must_use]
pub fn all_finite(values: &[f32]) -> bool {
    let mut lanes = [true; LANES];
    let mut chunks = values.chunks_exact(LANES);
    for chunk in &mut chunks {
        for (lane, value) in lanes.iter_mut().zip(chunk) {
            *lane &= value.to_bits() & EXPONENT_MASK != EXPONENT_MASK;
        }
    }
    let mut finite = lanes.iter().all(|lane| *lane);
    for value in chunks.remainder() {
        finite &= value.to_bits() & EXPONENT_MASK != EXPONENT_MASK;
    }
    finite
}

/// Whether every value is finite and at or above zero.
///
/// The radius and weight columns need both conditions, and testing them in one
/// sweep halves the memory traffic against two passes.
#[must_use]
pub fn all_finite_non_negative(values: &[f32]) -> bool {
    let mut lanes = [true; LANES];
    let mut chunks = values.chunks_exact(LANES);
    for chunk in &mut chunks {
        for (lane, value) in lanes.iter_mut().zip(chunk) {
            *lane &= value.to_bits() & EXPONENT_MASK != EXPONENT_MASK && *value >= 0.0;
        }
    }
    let mut valid = lanes.iter().all(|lane| *lane);
    for value in chunks.remainder() {
        valid &= value.to_bits() & EXPONENT_MASK != EXPONENT_MASK && *value >= 0.0;
    }
    valid
}

/// Whether every value is finite and inside an inclusive range.
#[must_use]
pub fn all_within(values: &[f32], low: f32, high: f32) -> bool {
    let mut lanes = [true; LANES];
    let mut chunks = values.chunks_exact(LANES);
    for chunk in &mut chunks {
        for (lane, value) in lanes.iter_mut().zip(chunk) {
            *lane &= *value >= low && *value <= high;
        }
    }
    let mut inside = lanes.iter().all(|lane| *lane);
    for value in chunks.remainder() {
        inside &= *value >= low && *value <= high;
    }
    inside
}

/// Smallest and largest finite value, as `(min, max)`.
///
/// Non-finite entries are skipped rather than poisoning the range, matching
/// the bound builders. An input with no finite value returns
/// `(f32::INFINITY, f32::NEG_INFINITY)` — the empty range, which is the
/// identity for a later merge.
#[must_use]
pub fn min_max(values: &[f32]) -> (f32, f32) {
    let mut low = [f32::INFINITY; LANES];
    let mut high = [f32::NEG_INFINITY; LANES];
    let mut chunks = values.chunks_exact(LANES);
    for chunk in &mut chunks {
        for ((low, high), value) in low.iter_mut().zip(high.iter_mut()).zip(chunk) {
            // `min`/`max` on a NaN operand return the other operand, so a
            // non-finite entry leaves the running range untouched.
            *low = low.min(*value);
            *high = high.max(*value);
        }
    }
    let mut minimum = f32::INFINITY;
    let mut maximum = f32::NEG_INFINITY;
    for (low, high) in low.iter().zip(&high) {
        minimum = minimum.min(*low);
        maximum = maximum.max(*high);
    }
    for value in chunks.remainder() {
        minimum = minimum.min(*value);
        maximum = maximum.max(*value);
    }
    if minimum.is_infinite() && maximum.is_infinite() {
        return (f32::INFINITY, f32::NEG_INFINITY);
    }
    (minimum, maximum)
}

/// Tightest bound over a position column, skipping non-finite rows.
#[must_use]
pub fn points_aabb(points: &[[f32; 3]]) -> Aabb {
    let mut low = [[f32::INFINITY; 3]; LANES];
    let mut high = [[f32::NEG_INFINITY; 3]; LANES];
    let mut chunks = points.chunks_exact(LANES);
    for chunk in &mut chunks {
        for ((low, high), point) in low.iter_mut().zip(high.iter_mut()).zip(chunk) {
            for ((low, high), value) in low.iter_mut().zip(high.iter_mut()).zip(point) {
                *low = low.min(*value);
                *high = high.max(*value);
            }
        }
    }
    let mut bounds = Aabb::EMPTY;
    for (low, high) in low.iter().zip(&high) {
        bounds.min = bounds.min.min(Vec3::from_array(*low));
        bounds.max = bounds.max.max(Vec3::from_array(*high));
    }
    for point in chunks.remainder() {
        bounds.extend(Vec3::from_array(*point));
    }
    if bounds.is_empty() {
        Aabb::EMPTY
    } else {
        bounds
    }
}

/// Tightest bound over spheres given as a position column and a radius column.
///
/// The two columns are consumed together so each is read once. A row whose
/// position or radius is not finite is skipped, so one bad atom cannot poison a
/// whole structure's bound.
#[must_use]
pub fn spheres_aabb(centers: &[[f32; 3]], radii: &[f32]) -> Aabb {
    let len = centers.len().min(radii.len());
    let (centers, radii) = (&centers[..len], &radii[..len]);
    let mut low = [[f32::INFINITY; 3]; LANES];
    let mut high = [[f32::NEG_INFINITY; 3]; LANES];
    let mut center_chunks = centers.chunks_exact(LANES);
    let mut radius_chunks = radii.chunks_exact(LANES);
    for (centers, radii) in (&mut center_chunks).zip(&mut radius_chunks) {
        for (((low, high), center), radius) in
            low.iter_mut().zip(high.iter_mut()).zip(centers).zip(radii)
        {
            let extent = radius.abs();
            for ((low, high), value) in low.iter_mut().zip(high.iter_mut()).zip(center) {
                *low = low.min(*value - extent);
                *high = high.max(*value + extent);
            }
        }
    }
    let mut bounds = Aabb::EMPTY;
    for (low, high) in low.iter().zip(&high) {
        bounds.min = bounds.min.min(Vec3::from_array(*low));
        bounds.max = bounds.max.max(Vec3::from_array(*high));
    }
    for (center, radius) in center_chunks
        .remainder()
        .iter()
        .zip(radius_chunks.remainder())
    {
        bounds.extend_sphere(Vec3::from_array(*center), *radius);
    }
    if bounds.is_empty() {
        Aabb::EMPTY
    } else {
        bounds
    }
}

#[cfg(test)]
#[path = "simd_tests.rs"]
mod tests;
