//! Where a ribbon starts and stops.
//!
//! A flat ribbon is a box, and a box drawn without its two end faces is open.
//! Nothing culls back faces here, so an open end does not read as a hole: it
//! reads as the inside of the far wall lit from the wrong side, which looks less
//! like missing surface than like a shading fault.

use super::cross_section::{SQUARE_CORNERS, cross_section};
use super::profiles::{profile_color, profile_extents_for_guide, sample_profile};
use super::ribbon::{
    PROFILE_SIDES, RibbonBuild, RibbonDeformation, RibbonVertex, SplineProfile, control_rows,
};
use super::traces::GuideKind;
use molgfx_core::SecondaryStructure;
use molgfx_math::{CurveSample, TransportFrame};

/// One end of a ribbon: the sample it stops on and which way it faces.
#[derive(Clone, Copy, Debug)]
struct RibbonEnd {
    sample: CurveSample,
    frame: TransportFrame,
    /// `1.0` at the trailing end of the trace, `-1.0` at the leading one.
    outward: f32,
}

/// Closes both ends with faces matching the authored cross-section.
///
/// Nothing culls back faces here, so an open end does not read as a hole: it
/// reads as the inside of the far wall lit from the wrong side, which looks less
/// like missing surface than like a shading fault. The cap gets its own vertices
/// so it can carry the end's normal — the tangent — rather than inheriting the
/// side normals it would get by reusing the ring, which shades a blunt tip as if
/// it were the ribbon's edge at exactly the place the eye picks the ribbon up.
pub(super) fn append_end_caps(
    entities: &[u32],
    styles: &[SecondaryStructure],
    guides: &[GuideKind],
    width: f32,
    thickness: f32,
    build: &mut RibbonBuild<'_>,
) -> Result<(), crate::PackingError> {
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
        _ => return Ok(()),
    };
    let Some(first_ring) = build
        .deformations
        .len()
        .checked_sub(build.samples.len() * PROFILE_SIDES)
    else {
        return Ok(());
    };
    let recipes = [
        build.deformations.get(first_ring).copied(),
        build.deformations.last().copied(),
    ];
    for (end, recipe) in ends.into_iter().zip(recipes) {
        if build.params.profile == SplineProfile::Twister {
            append_cap(end, entities, width, thickness, build)?;
        } else if let Some(recipe) = recipe {
            append_round_cap(end, entities, styles, guides, recipe, build)?;
        }
    }
    Ok(())
}

fn append_cap(
    end: RibbonEnd,
    entities: &[u32],
    width: f32,
    thickness: f32,
    build: &mut RibbonBuild<'_>,
) -> Result<(), crate::PackingError> {
    let segment = usize::try_from(end.sample.segment)
        .into_iter()
        .fold(usize::MAX, |_, value| value);
    let entity_id = match entities.get(segment) {
        Some(&value) => value,
        None => u32::MAX,
    };
    let controls = control_rows(entities, segment);
    let normal = end.frame.tangent * end.outward;
    let base = super::draw_limits::vertex_index(build.vertices.len())?;
    for [x, y] in SQUARE_CORNERS {
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
    Ok(())
}

fn append_round_cap(
    end: RibbonEnd,
    entities: &[u32],
    styles: &[SecondaryStructure],
    guides: &[GuideKind],
    recipe: RibbonDeformation,
    build: &mut RibbonBuild<'_>,
) -> Result<(), crate::PackingError> {
    let Ok(segment) = usize::try_from(end.sample.segment) else {
        return Ok(());
    };
    let Some(&entity_id) = entities.get(segment) else {
        return Ok(());
    };
    let style = match styles.get(segment) {
        Some(&style) => style,
        None => SecondaryStructure::Unknown,
    };
    let (width, thickness) = profile_extents_for_guide(
        build.params,
        style,
        end.sample.parameter,
        styles,
        segment,
        guides.get(segment).copied(),
    );
    let base = super::draw_limits::vertex_index(build.vertices.len())?;
    let normal = (end.frame.tangent * end.outward).to_array();
    build.vertices.push(RibbonVertex {
        position: end.sample.position.to_array(),
        entity_id,
        normal,
        color: build.params.color,
    });
    build.deformations.push(recipe);
    for side in 0..PROFILE_SIDES {
        let (x, y, _) = cross_section(
            sample_profile(build.params, style, guides.get(segment).copied()),
            side,
            width,
            thickness,
        );
        let position = end.sample.position
            + end.frame.normal * (x * width)
            + end.frame.binormal * (y * thickness);
        build.vertices.push(RibbonVertex {
            position: position.to_array(),
            entity_id,
            normal,
            color: build.params.color,
        });
        build.deformations.push(recipe);
    }
    for side in 0..PROFILE_SIDES {
        let current = super::draw_limits::vertex_index(side + 1)?;
        let next = super::draw_limits::vertex_index((side + 1) % PROFILE_SIDES + 1)?;
        let triangle = if end.outward > 0.0 {
            [base, base + current, base + next]
        } else {
            [base, base + next, base + current]
        };
        build.indices.extend_from_slice(&triangle);
    }
    Ok(())
}

#[cfg(test)]
#[path = "ends_tests.rs"]
mod tests;
