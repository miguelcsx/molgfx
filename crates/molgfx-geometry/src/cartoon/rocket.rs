//! Straight helix columns fitted against sequence order in O(guides).

use molgfx_core::SecondaryStructure;
use molgfx_math::Vec3;
use num_traits::ToPrimitive;
use std::ops::Range;

const MINIMUM_HELIX_GUIDES: usize = 2;

pub(super) fn project_trace(
    points: &[Vec3],
    styles: &[SecondaryStructure],
    output: &mut Vec<Vec3>,
) {
    output.resize(points.len(), Vec3::ZERO);
    project_trace_into(points, styles, output);
}

pub(super) fn project_trace_into(
    points: &[Vec3],
    styles: &[SecondaryStructure],
    output: &mut [Vec3],
) {
    output.copy_from_slice(points);
    let mut strand = 0;
    while strand < points.len() {
        if !styles
            .get(strand)
            .is_some_and(|style| style.is_sheet_like())
        {
            strand += 1;
            continue;
        }
        let mut end = strand + 1;
        while end < points.len() && styles.get(end).is_some_and(|style| style.is_sheet_like()) {
            end += 1;
        }
        if end - strand >= 3 {
            output[strand] = points[strand] + points[strand + 1] * 0.5 - points[strand + 2] * 0.5;
            output[end - 1] = points[end - 1] + points[end - 2] * 0.5 - points[end - 3] * 0.5;
        }
        strand = end;
    }
    let mut start = 0;
    while start < points.len() {
        let Some(run) = helix_run(styles, start) else {
            start += 1;
            continue;
        };
        let end = run.end.min(points.len());
        if end - start >= MINIMUM_HELIX_GUIDES {
            project(&points[start..end], &mut output[start..end]);
            let slope = output[start + 1] - output[start];
            output[start] -= slope * 0.5;
            output[end - 1] += slope * 0.5;
        }
        start = end;
    }
}

pub(super) fn filter_strands(
    points: &[Vec3],
    styles: &[SecondaryStructure],
    output: &mut Vec<Vec3>,
) {
    output.resize(points.len(), Vec3::ZERO);
    filter_strands_into(points, styles, output);
}

fn filter_strands_into(points: &[Vec3], styles: &[SecondaryStructure], output: &mut [Vec3]) {
    output.copy_from_slice(points);
    for (index, point) in output.iter_mut().enumerate() {
        if styles.get(index).is_some_and(|style| style.is_strand()) {
            let previous = points[index.saturating_sub(1)];
            let next = points[(index + 1).min(points.len() - 1)];
            *point = previous * 0.25 + points[index] * 0.5 + next * 0.25;
        }
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
    let Some((center, slope)) = helix_fit(points) else {
        return;
    };
    let Some(count) = points.len().to_f32() else {
        return;
    };
    let midpoint = (count - 1.0) * 0.5;
    for (index, output) in output.iter_mut().enumerate() {
        let Some(index) = index.to_f32() else {
            return;
        };
        *output = center + slope * (index - midpoint);
    }
}

pub(super) fn helix_fit(points: &[Vec3]) -> Option<(Vec3, Vec3)> {
    if points.len() < 2 {
        return None;
    }
    let count = points.len().to_f32()?;
    let center = points.iter().copied().sum::<Vec3>() / count;
    let midpoint = (count - 1.0) * 0.5;
    let mut numerator = Vec3::ZERO;
    let mut denominator = 0.0;
    for (index, point) in points.iter().enumerate() {
        let index = index.to_f32()?;
        let offset = index - midpoint;
        numerator += (*point - center) * offset;
        denominator += offset * offset;
    }
    Some((center, numerator / denominator))
}

#[cfg(test)]
#[path = "rocket_tests.rs"]
mod tests;
