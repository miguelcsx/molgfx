//! Where a ribbon starts and stops.
//!
//! A flat ribbon is a box, and a box drawn without its two end faces is open.
//! Nothing culls back faces here, so an open end does not read as a hole: it
//! reads as the inside of the far wall lit from the wrong side, which looks less
//! like missing surface than like a shading fault.

use super::profiles::{BOX_CORNERS, profile_color};
use super::ribbon::{RibbonBuild, RibbonDeformation, RibbonVertex, SplineProfile, control_rows};
use molgfx_math::{CurveSample, TransportFrame};

/// One end of a ribbon: the sample it stops on and which way it faces.
#[derive(Clone, Copy, Debug)]
struct RibbonEnd {
    sample: CurveSample,
    frame: TransportFrame,
    /// `1.0` at the trailing end of the trace, `-1.0` at the leading one.
    outward: f32,
}

/// Closes the two open ends of a flat ribbon.
///
/// Nothing culls back faces here, so an open end does not read as a hole: it
/// reads as the inside of the far wall lit from the wrong side, which looks less
/// like missing surface than like a shading fault. The cap gets its own vertices
/// so it can carry the end's normal — the tangent — rather than inheriting the
/// side normals it would get by reusing the ring, which shades a blunt tip as if
/// it were the ribbon's edge at exactly the place the eye picks the ribbon up.
pub(super) fn append_end_caps(
    entities: &[u32],
    width: f32,
    thickness: f32,
    build: &mut RibbonBuild<'_>,
) {
    let ends = match (
        build.samples.first().copied(),
        build.frames.first().copied(),
        build.samples.last().copied(),
        build.frames.last().copied(),
    ) {
        (Some(first), Some(first_frame), Some(last), Some(last_frame)) => [
            RibbonEnd {
                sample: first,
                frame: first_frame,
                outward: -1.0,
            },
            RibbonEnd {
                sample: last,
                frame: last_frame,
                outward: 1.0,
            },
        ],
        _ => return,
    };
    for end in ends {
        append_cap(end, entities, width, thickness, build);
    }
}

fn append_cap(
    end: RibbonEnd,
    entities: &[u32],
    width: f32,
    thickness: f32,
    build: &mut RibbonBuild<'_>,
) {
    let segment = usize::try_from(end.sample.segment).map_or(usize::MAX, |value| value);
    let entity_id = match entities.get(segment) {
        Some(&value) => value,
        None => u32::MAX,
    };
    let controls = control_rows(entities, segment);
    let normal = end.frame.tangent * end.outward;
    let Ok(base) = u32::try_from(build.vertices.len()) else {
        return;
    };
    for [x, y] in BOX_CORNERS {
        let offset = end.frame.normal * (x * width) + end.frame.binormal * (y * thickness);
        build.vertices.push(RibbonVertex {
            position: (end.sample.position + offset).to_array(),
            entity_id,
            normal: normal.to_array(),
            color: profile_color(SplineProfile::Twister, y, build.params.color),
        });
        build.deformations.push(RibbonDeformation {
            controls,
            parameter: [end.sample.parameter, 0.0, 0.0, 0.0],
        });
    }
    // Wind the fan out of the end it closes, so the cap stays correct if back
    // faces are ever culled.
    let quad = if end.outward > 0.0 {
        [base, base + 1, base + 2, base, base + 2, base + 3]
    } else {
        [base, base + 2, base + 1, base, base + 3, base + 2]
    };
    build.indices.extend_from_slice(&quad);
}
