//! Peptide oxygen orientation in O(samples), with transport as the fallback.

use molgfx_math::{CurveSample, TransportFrame, Vec3};

#[cfg(test)]
#[path = "orientation_tests.rs"]
mod tests;

pub(super) fn orient_to_peptides(
    samples: &[CurveSample],
    oxygen_directions: &[Vec3],
    frames: &mut [TransportFrame],
) {
    let mut previous = None;
    for (sample, frame) in samples.iter().zip(frames) {
        let segment = sample.segment as usize;
        let Some(&left) = oxygen_directions.get(segment) else {
            continue;
        };
        let Some(&right) = oxygen_directions.get(segment + 1) else {
            continue;
        };
        let Some(mut normal) = projected_blend(left, right, sample.parameter, frame.tangent) else {
            continue;
        };
        if previous.is_some_and(|value: Vec3| value.dot(normal) < 0.0) {
            normal = -normal;
        }
        frame.normal = normal;
        frame.binormal = frame.tangent.cross(normal);
        previous = Some(normal);
    }
}

fn projected_blend(left: Vec3, right: Vec3, amount: f32, tangent: Vec3) -> Option<Vec3> {
    let left = (left - tangent * left.dot(tangent)).try_normalize()?;
    let mut right = (right - tangent * right.dot(tangent)).try_normalize()?;
    if left.dot(right) < 0.0 {
        right = -right;
    }
    left.lerp(right, amount).try_normalize()
}
