//! Source-backed connectors across genuine polymer discontinuities.
//!
//! Topology is visited once, O(residues × selection membership cost). Geometry costs O(dashes), with reused
//! spline scratch. Resident trajectory endpoints bound the maximum dash count;
//! time-only updates deform the existing vertices without CPU traversal.

use super::ribbon::{RibbonBuild, SplineProfile};
use super::sweep::{RibbonTrace, append_ribbon};
use super::traces::{CARTOON_GAP_CUTOFF, GuideKind, polymer_guide};
use crate::CartoonError;
use molgfx_core::{AtomSelection, EntityId, EntityKind, TrajectorySegment};
use molgfx_math::Vec3;

const DASH_LENGTH: f32 = 0.5;
const DASH_SPACING: f32 = 0.5;
const GAP_RADIUS: f32 = 0.15;
const GAP_ANCHOR: u32 = 2;

#[derive(Clone, Copy)]
struct Guide {
    row: u32,
    position: Vec3,
    kind: GuideKind,
    sequence: Option<i32>,
}

pub(super) fn append_gaps(
    structure: &molframe::Structure,
    selection: &AtomSelection,
    trajectory: Option<&TrajectorySegment>,
    build: &mut RibbonBuild<'_>,
) -> Result<(), CartoonError> {
    for chain in structure.chains() {
        let mut previous: Option<Guide> = None;
        let mut missing_guide = false;
        for residue in chain.residues() {
            let Some((atom, kind)) = polymer_guide(residue) else {
                missing_guide = true;
                continue;
            };
            if !selection.contains(atom.index().get()) {
                previous = None;
                missing_guide = false;
                continue;
            }
            let Some(position) = atom.position().map(Vec3::from) else {
                missing_guide = true;
                continue;
            };
            let current = Guide {
                row: atom.index().get(),
                position,
                kind,
                sequence: residue.label_seq_id(),
            };
            if let Some(left) = previous
                && left.kind == current.kind
                && (missing_guide || is_gap(left, current))
            {
                append_connector(left, current, trajectory, build)?;
            }
            previous = Some(current);
            missing_guide = false;
        }
    }
    Ok(())
}

fn is_gap(left: Guide, right: Guide) -> bool {
    let missing_sequence = super::traces::missing_sequence_interval(left.sequence, right.sequence);
    missing_sequence || left.position.distance_squared(right.position) > CARTOON_GAP_CUTOFF.powi(2)
}

fn maximum_length(left: Guide, right: Guide, trajectory: Option<&TrajectorySegment>) -> f32 {
    let mut length = left.position.distance(right.position);
    if let Some(segment) = trajectory {
        for frame in [segment.start(), segment.end()] {
            if let (Some(left), Some(right)) = (
                frame.positions().get(left.row as usize),
                frame.positions().get(right.row as usize),
            ) {
                length = length.max(Vec3::from_array(*left).distance(Vec3::from_array(*right)));
            }
        }
    }
    length
}

fn append_connector(
    left: Guide,
    right: Guide,
    trajectory: Option<&TrajectorySegment>,
    build: &mut RibbonBuild<'_>,
) -> Result<(), CartoonError> {
    let Some(direction) = (right.position - left.position).try_normalize() else {
        return Err(CartoonError::GapResolution);
    };
    let length = maximum_length(left, right, trajectory);
    if !length.is_finite() {
        return Err(CartoonError::GapResolution);
    }
    let entities = [
        EntityId::pack(EntityKind::Atom, u64::from(left.row))
            .map_err(crate::PackingError::from)?
            .0,
        EntityId::pack(EntityKind::Atom, u64::from(right.row))
            .map_err(crate::PackingError::from)?
            .0,
    ];
    let mut distance = 0.0;
    let period = DASH_LENGTH + DASH_SPACING;
    super::draw_limits::check_gap_capacity(
        build.vertices.len(),
        build.indices.len(),
        length,
        period,
    )?;
    let original_params = build.params;
    build.params.width = GAP_RADIUS * 2.0;
    build.params.thickness = GAP_RADIUS * 2.0;
    build.params.profile = SplineProfile::Tube;
    // Every dash is straight, so two endpoint samples represent it exactly.
    build.params.max_steps = 1;
    build.params.direction_wedges = false;
    let result = (|| {
        while distance < length {
            let end = (distance + DASH_LENGTH).min(length);
            let points = [
                left.position + direction * distance,
                left.position + direction * end,
            ];
            let first_vertex = build.vertices.len();
            let first_recipe = build.deformations.len();
            append_ribbon(
                RibbonTrace {
                    points: &points,
                    entities: &entities,
                    styles: &[],
                    property_base: None,
                    property_count: 0,
                    normals: &[],
                    guides: &[],
                },
                build,
            )?;
            for (vertex, recipe) in build.vertices[first_vertex..]
                .iter()
                .zip(&mut build.deformations[first_recipe..])
            {
                recipe.controls = [left.row, right.row, left.row, right.row];
                recipe.parameter = [
                    (Vec3::from_array(vertex.position) - left.position).dot(direction),
                    distance,
                    0.0,
                    f32::from_bits(GAP_ANCHOR),
                ];
            }
            let next = distance + period;
            if next <= distance {
                return Err(CartoonError::GapResolution);
            }
            distance = next;
        }
        Ok(())
    })();
    build.params = original_params;
    result
}

#[cfg(test)]
#[path = "gaps_tests.rs"]
mod tests;
