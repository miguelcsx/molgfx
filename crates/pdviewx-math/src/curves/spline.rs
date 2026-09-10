//! Deterministic Catmull-Rom evaluation and curvature-adaptive sampling.

use crate::Vec3;

#[cfg(test)]
#[path = "spline_tests.rs"]
mod tests;

/// One sampled point and unit tangent along a spline.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CurveSample {
    /// Evaluated position.
    pub position: Vec3,
    /// Unit direction of increasing curve parameter.
    pub tangent: Vec3,
    /// Source control-point interval.
    pub segment: u32,
    /// Local parameter within the source interval.
    pub parameter: f32,
}

/// Samples a centripetal-looking uniform Catmull-Rom trace with more samples
/// where midpoint deviation and tangent change are high. Output storage is
/// caller-owned and reused; work and output are bounded by `max_steps`.
pub fn sample_catmull_rom(
    points: &[Vec3],
    tolerance: f32,
    max_steps: u8,
    out: &mut Vec<CurveSample>,
) {
    sample_catmull_rom_demanding(points, tolerance, max_steps, &[], out);
}

/// Adaptive sampling with one extra per-interval demand the curve cannot see.
///
/// Deviation and tangent turn describe the shape of the curve, and nothing
/// else about a ribbon reaches this function. A ribbon that also twists along
/// the curve needs samples the shape never asks for: a glycan runs almost
/// straight between two sugars while the ring plane rotates through as much as
/// a half turn, and it would otherwise be handed one sample per sugar and have
/// to do that rotation inside a single quad, which folds. Passing the twist as
/// a demand — in units of samples — buys density exactly where the ribbon turns
/// instead of everywhere, which is the difference between a fixed dense trace
/// and one that costs what it needs.
///
/// `demand[i]` applies to the interval from control point `i` to `i + 1`; a
/// shorter or empty slice contributes nothing.
pub fn sample_catmull_rom_demanding(
    points: &[Vec3],
    tolerance: f32,
    max_steps: u8,
    demand: &[f32],
    out: &mut Vec<CurveSample>,
) {
    out.clear();
    if points.len() < 2 {
        if let Some(&position) = points.first() {
            out.push(CurveSample {
                position,
                tangent: Vec3::Z,
                segment: 0,
                parameter: 0.0,
            });
        }
        return;
    }
    let maximum = max_steps.max(1);
    for segment in 0..points.len() - 1 {
        let curve = segment_points(points, segment);
        let extra = match demand.get(segment) {
            Some(value) if value.is_finite() => value.max(0.0),
            _ => 0.0,
        };
        let steps = adaptive_steps(curve, tolerance.max(1e-4), maximum, extra);
        let start = u8::from(segment != 0);
        for step in start..=steps {
            let t = f32::from(step) / f32::from(steps);
            let derivative = tangent(curve, t);
            out.push(CurveSample {
                position: position(curve, t),
                tangent: normalized(derivative, Vec3::Z),
                segment: u32::try_from(segment).map_or(u32::MAX, |value| value),
                parameter: t,
            });
        }
    }
}

/// Fixed-step Catmull-Rom sampling: the form a GPU kernel evaluates in
/// parallel, one invocation per sample. The basis is identical to
/// [`sample_catmull_rom`] — only the per-segment step count is a constant
/// instead of curvature-adaptive — so this doubles as the CPU parity reference
/// for the `spline` compute shader. Output storage is caller-owned and reused.
pub fn sample_catmull_rom_fixed(
    points: &[Vec3],
    steps_per_segment: u8,
    out: &mut Vec<CurveSample>,
) {
    out.clear();
    if points.len() < 2 {
        if let Some(&position) = points.first() {
            out.push(CurveSample {
                position,
                tangent: Vec3::Z,
                segment: 0,
                parameter: 0.0,
            });
        }
        return;
    }
    let steps = steps_per_segment.max(1);
    for segment in 0..points.len() - 1 {
        let curve = segment_points(points, segment);
        let start = u8::from(segment != 0);
        for step in start..=steps {
            let t = f32::from(step) / f32::from(steps);
            out.push(CurveSample {
                position: position(curve, t),
                tangent: normalized(tangent(curve, t), Vec3::Z),
                segment: u32::try_from(segment).map_or(u32::MAX, |value| value),
                parameter: t,
            });
        }
    }
}

#[inline]
fn segment_points(points: &[Vec3], segment: usize) -> [Vec3; 4] {
    let last = points.len() - 1;
    [
        points[segment.saturating_sub(1)],
        points[segment],
        points[(segment + 1).min(last)],
        points[(segment + 2).min(last)],
    ]
}

#[inline]
fn adaptive_steps(points: [Vec3; 4], tolerance: f32, maximum: u8, extra: f32) -> u8 {
    let midpoint = position(points, 0.5);
    let chord_midpoint = (points[1] + points[2]) * 0.5;
    let deviation = midpoint.distance(chord_midpoint);
    let start = normalized(tangent(points, 0.0), Vec3::Z);
    let end = normalized(tangent(points, 1.0), start);
    let turn = start.dot(end).clamp(-1.0, 1.0).acos();
    let demand = (deviation / tolerance).sqrt() + turn * 2.0 + extra;
    let mut steps = 1u8;
    while steps < maximum && f32::from(steps) < demand {
        steps = steps.saturating_mul(2).min(maximum);
    }
    steps
}

#[inline]
fn normalized(value: Vec3, fallback: Vec3) -> Vec3 {
    match value.try_normalize() {
        Some(unit) => unit,
        None => fallback,
    }
}

#[inline]
fn position(points: [Vec3; 4], t: f32) -> Vec3 {
    let [p0, p1, p2, p3] = points;
    let t2 = t * t;
    let t3 = t2 * t;
    (p1 * 2.0
        + (p2 - p0) * t
        + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
        + (-p0 + p1 * 3.0 - p2 * 3.0 + p3) * t3)
        * 0.5
}

#[inline]
fn tangent(points: [Vec3; 4], t: f32) -> Vec3 {
    let [p0, p1, p2, p3] = points;
    let t2 = t * t;
    ((p2 - p0)
        + (p0 * 4.0 - p1 * 10.0 + p2 * 8.0 - p3 * 2.0) * t
        + (-p0 * 3.0 + p1 * 9.0 - p2 * 9.0 + p3 * 3.0) * t2)
        * 0.5
}
