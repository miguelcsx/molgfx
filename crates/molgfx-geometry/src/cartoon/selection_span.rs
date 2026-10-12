//! Clips an already sampled curve without changing its source control points.

use crate::PackingError;
use molgfx_math::{CurveSample, Vec3, catmull_rom_position, catmull_rom_tangent};
use num_traits::ToPrimitive as _;

pub(super) fn entity_index(segment: usize, parameter: f32, range: Option<[f32; 2]>) -> usize {
    if range.is_none() {
        return segment;
    }
    let end_cut = range.is_some_and(|[_, maximum]| {
        parameter.to_bits() == 0.5_f32.to_bits()
            && maximum.fract().to_bits() == 0.5_f32.to_bits()
            && maximum.to_usize() == Some(segment)
    });
    segment + usize::from(parameter >= 0.5 && !end_cut)
}

pub(super) fn clip_samples(
    points: &[Vec3],
    range: [f32; 2],
    linear: bool,
    samples: &mut Vec<CurveSample>,
) -> Result<(), PackingError> {
    let first = boundary(points, range[0], linear)?;
    let last = boundary(points, range[1], linear)?;
    samples.retain(|sample| {
        sample.segment.to_f32().is_some_and(|segment| {
            let parameter = segment + sample.parameter;
            parameter > range[0] && parameter < range[1]
        })
    });
    samples.insert(0, first);
    samples.push(last);
    Ok(())
}

fn boundary(points: &[Vec3], value: f32, linear: bool) -> Result<CurveSample, PackingError> {
    let Some(last) = points.len().checked_sub(1).filter(|last| *last > 0) else {
        return Err(PackingError::Topology {
            operation: "selected polymer span",
        });
    };
    let segment = value
        .to_usize()
        .ok_or(PackingError::Topology {
            operation: "selected polymer parameter",
        })?
        .min(last - 1);
    let parameter = value
        - segment.to_f32().ok_or(PackingError::Topology {
            operation: "selected polymer segment",
        })?;
    let controls = [
        points[segment.saturating_sub(1)],
        points[segment],
        points[segment + 1],
        points[(segment + 2).min(last)],
    ];
    let (position, tangent) = if linear {
        (
            controls[1].lerp(controls[2], parameter),
            controls[2] - controls[1],
        )
    } else {
        (
            catmull_rom_position(controls, parameter),
            catmull_rom_tangent(controls, parameter),
        )
    };
    Ok(CurveSample {
        position,
        tangent: tangent.normalize_or_zero(),
        segment: super::draw_limits::vertex_index(segment)?,
        parameter,
    })
}
