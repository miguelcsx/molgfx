//! Deterministic cubic Bezier evaluation.

use crate::Vec3;

#[cfg(test)]
#[path = "bezier_tests.rs"]
mod tests;

/// Samples a cubic Bezier segment, including both endpoints.
///
/// Output storage is caller-owned and reused. `segments` is clamped to at
/// least one, so the result always contains exactly `segments + 1` points.
pub fn sample_cubic_bezier(control: [Vec3; 4], segments: u16, out: &mut Vec<Vec3>) {
    let segments = segments.max(1);
    out.clear();
    out.reserve(usize::from(segments) + 1);
    for step in 0..=segments {
        let t = f32::from(step) / f32::from(segments);
        let one_minus_t = 1.0 - t;
        out.push(
            control[0] * one_minus_t.powi(3)
                + control[1] * (3.0 * one_minus_t.powi(2) * t)
                + control[2] * (3.0 * one_minus_t * t * t)
                + control[3] * t.powi(3),
        );
    }
}
