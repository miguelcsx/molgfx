//! Straight helix columns fitted in O(guides), with a fixed iteration budget.

use molgfx_core::SecondaryStructure;
use molgfx_math::Vec3;
use num_traits::ToPrimitive;
use std::ops::Range;

const AXIS_ITERATIONS: usize = 12;
const MINIMUM_HELIX_GUIDES: usize = 4;

pub(super) fn project_trace(
    points: &[Vec3],
    styles: &[SecondaryStructure],
    output: &mut Vec<Vec3>,
) {
    output.clear();
    output.extend_from_slice(points);
    let mut start = 0;
    while start < points.len() {
        let Some(run) = helix_run(styles, start) else {
            start += 1;
            continue;
        };
        let end = run.end.min(points.len());
        if end - start >= MINIMUM_HELIX_GUIDES {
            project(&points[start..end], &mut output[start..end]);
        }
        start = end;
    }
}

pub(super) fn helix_run(styles: &[SecondaryStructure], start: usize) -> Option<Range<usize>> {
    if !styles.get(start).is_some_and(|style| style.is_helix()) {
        return None;
    }
    let mut first = start;
    while first > 0 && styles[first - 1].is_helix() {
        first -= 1;
    }
    let mut end = start + 1;
    while end < styles.len() && styles[end].is_helix() {
        end += 1;
    }
    (end - first >= MINIMUM_HELIX_GUIDES).then_some(first..end)
}

fn project(points: &[Vec3], output: &mut [Vec3]) {
    let Some(count) = points.len().to_f32() else {
        return;
    };
    let center = points.iter().copied().sum::<Vec3>() / count;
    let Some(mut axis) = (points[points.len() - 1] - points[0]).try_normalize() else {
        return;
    };
    for _ in 0..AXIS_ITERATIONS {
        let covariance = points
            .iter()
            .map(|point| {
                let relative = *point - center;
                relative * relative.dot(axis)
            })
            .sum::<Vec3>();
        let Some(next) = covariance.try_normalize() else {
            return;
        };
        axis = next;
    }
    // Source order fixes the sense of the axis and each guide's projected location.
    if axis.dot(points[points.len() - 1] - points[0]) < 0.0 {
        axis = -axis;
    }
    for (point, output) in points.iter().zip(output) {
        *output = center + axis * (*point - center).dot(axis);
    }
}
