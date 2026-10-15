//! Pixel-width polymer strokes with live GPU guide coordinates. O(intervals).

use super::ribbon::{RibbonBuild, RibbonDeformation, RibbonVertex, SplineProfile, control_rows};
use super::sweep::RibbonTrace;
use crate::PackingError;
use molgfx_math::{Vec3, catmull_rom_position};

pub(super) fn append(
    input: RibbonTrace<'_>,
    build: &mut RibbonBuild<'_>,
) -> Result<(), PackingError> {
    if input.entities.len() != input.points.len() {
        return Err(PackingError::Topology {
            operation: "polymer stroke guide identities",
        });
    }
    build.samples.clear();
    build.frames.clear();
    let smooth = build.params.profile == SplineProfile::Tube;
    let steps = if smooth { 6u8 } else { 1u8 };
    let segments = input
        .points
        .len()
        .saturating_sub(1)
        .saturating_mul(usize::from(steps));
    if segments
        .saturating_mul(4)
        .saturating_add(build.vertices.len())
        > 4_000_000
    {
        return Err(PackingError::GeometryBudget {
            resource: "polymer stroke vertices",
        });
    }
    let _ = super::draw_limits::vertex_index(
        build
            .indices
            .len()
            .saturating_add(segments.saturating_mul(6)),
    )?;
    for interval in 0..input.points.len().saturating_sub(1) {
        let offset = super::draw_limits::exact_f32(interval, "polymer stroke parameter")?;
        let (minimum, maximum) = match input.parameter_range {
            Some([minimum, maximum]) => ((minimum - offset).max(0.0), (maximum - offset).min(1.0)),
            None => (0.0, 1.0),
        };
        if minimum >= maximum {
            continue;
        }
        let controls = control_rows(input.entities, interval);
        let points = [
            input.points[interval.saturating_sub(1)],
            input.points[interval],
            input.points[interval + 1],
            input.points[(interval + 2).min(input.points.len() - 1)],
        ];
        for step in 0..steps {
            let t = [
                (f32::from(step) / f32::from(steps)).max(minimum),
                (f32::from(step + 1) / f32::from(steps)).min(maximum),
            ];
            if t[0] >= t[1] {
                continue;
            }
            let base = super::draw_limits::vertex_index(build.vertices.len())?;
            let _ = super::draw_limits::vertex_index(build.vertices.len() + 4)?;
            for (end, side) in [(0, -1.0), (0, 1.0), (1, -1.0), (1, 1.0)] {
                let position = if smooth {
                    catmull_rom_position(points, t[end])
                } else {
                    points[1].lerp(points[2], t[end])
                };
                build.vertices.push(RibbonVertex {
                    position: position.to_array(),
                    normal: Vec3::Z.to_array(),
                    entity_id: input.entities[match input.parameter_range {
                        Some(_) => super::selection_span::entity_index(
                            interval,
                            t[end],
                            input.parameter_range,
                        ),
                        None => interval + usize::from(t[end] > 0.5),
                    }],
                    color: build.params.color,
                });
                build.deformations.push(RibbonDeformation {
                    controls,
                    parameter: [
                        t[end],
                        side,
                        t[1 - end],
                        f32::from_bits(if smooth { 13 } else { 12 }),
                    ],
                });
            }
            build
                .indices
                .extend([base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "stroke_tests.rs"]
mod tests;
