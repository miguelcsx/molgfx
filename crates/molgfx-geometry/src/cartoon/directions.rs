//! Source-anchored polymer direction glyphs, generated in O(guides + samples).
//! Working trace/frame storage is reused; the GPU deforms each glyph through
//! the same resident source coordinates as its ribbon.

use super::error::CartoonError;
use super::profiles::profile_extents_for_guide;
use super::ribbon::{RibbonDeformation, RibbonParams, RibbonVertex, control_rows};
use super::sweep::RibbonTrace;
use super::traces::{GuideKind, PolymerTraces, extract_direction_traces};
use molgfx_core::{AtomSelection, EntityId, EntityKind, SecondaryStructure};
use molgfx_math::{CurveSample, TransportFrame, Vec3};

/// A glyph lies just above its ribbon face rather than competing with its depth.
const FACE_CLEARANCE: f32 = 0.02;
/// Half of the authored ribbon width determines the glyph's full width/length.
const WIDTH_FACTOR: f32 = 0.5;
/// The triangle's centroid remains over the guide rather than over its tip.
const TIP_FRACTION: f32 = 2.0 / 3.0;
/// An isolated residue anchors at its guide and uses its own backbone atoms.
const ATOM_ANCHOR: u32 = 1;

impl RibbonParams {
    /// Conservative distance from a source guide to any direction glyph vertex.
    /// Uses the same profile resolver as generation, including nucleic depth.
    #[must_use]
    pub fn direction_glyph_radius(self) -> f32 {
        if !self.direction_wedges {
            return 0.0;
        }
        let depth = SecondaryStructure::ALL
            .into_iter()
            .flat_map(|style| {
                [GuideKind::AlphaCarbon, GuideKind::SugarCarbon]
                    .map(|guide| profile_extents_for_guide(self, style, 0.0, &[], 0, Some(guide)).1)
            })
            .fold(0.0_f32, f32::max);
        let length = self.width.abs() * WIDTH_FACTOR;
        (depth + FACE_CLEARANCE).hypot(length * TIP_FRACTION)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct DirectionStorage {
    traces: PolymerTraces,
    samples: Vec<CurveSample>,
    frames: Vec<TransportFrame>,
}

pub(super) struct DirectionBuild<'a> {
    pub(super) params: RibbonParams,
    pub(super) vertices: &'a mut Vec<RibbonVertex>,
    pub(super) indices: &'a mut Vec<u32>,
    pub(super) deformations: &'a mut Vec<RibbonDeformation>,
}

impl DirectionStorage {
    pub(super) fn generate(
        &mut self,
        structure: &molframe::Structure,
        selection: &AtomSelection,
        secondary: &[SecondaryStructure],
        max_gap: f32,
        build: &mut DirectionBuild<'_>,
    ) -> Result<(), CartoonError> {
        extract_direction_traces(structure, secondary, max_gap, &mut self.traces)?;
        for range in &self.traces.ranges {
            let points = range.points.clone();
            let input = RibbonTrace {
                points: &self.traces.points[points.clone()],
                entities: &self.traces.entities[points.clone()],
                styles: &self.traces.styles[points.clone()],
                guides: &self.traces.guides[points],
                property_base: None,
                property_count: 0,
                normals: &[],
            };
            sample(input, build.params, &mut self.samples, &mut self.frames);
            append_trace(
                input,
                &self.samples,
                &self.frames,
                Some((structure, selection)),
                build,
            )?;
        }
        Ok(())
    }
}

pub(super) fn sample(
    input: RibbonTrace<'_>,
    params: RibbonParams,
    samples: &mut Vec<CurveSample>,
    frames: &mut Vec<TransportFrame>,
) {
    molgfx_math::sample_catmull_rom(input.points, params.tolerance, params.max_steps, samples);
    molgfx_math::parallel_transport(samples, frames);
}

pub(super) fn append_trace(
    input: RibbonTrace<'_>,
    samples: &[CurveSample],
    frames: &[TransportFrame],
    source: Option<(&molframe::Structure, &AtomSelection)>,
    build: &mut DirectionBuild<'_>,
) -> Result<(), CartoonError> {
    for (sample, frame) in samples.iter().zip(frames) {
        let guide = if sample.segment == 0 && sample.parameter == 0.0 {
            0
        } else if sample.parameter.to_bits() == 1.0_f32.to_bits() {
            sample.segment as usize + 1
        } else {
            continue;
        };
        let Some(&entity) = input.entities.get(guide) else {
            return Err(CartoonError::GuideDirection { entity: u32::MAX });
        };
        if let Some((_, selection)) = source {
            let Some((EntityKind::Atom, row)) = EntityId(entity).unpack() else {
                return Err(CartoonError::GuideDirection { entity });
            };
            if !selection.contains(row) {
                continue;
            }
        }
        let (frame, recipe) = if input.points.len() == 1 {
            singleton(
                entity,
                input.guides.first().copied(),
                source.map(|(structure, _)| structure),
            )?
        } else {
            let previous = input.points[guide.saturating_sub(1)];
            let next = input.points[(guide + 1).min(input.points.len() - 1)];
            if (next - previous).try_normalize().is_none() {
                return Err(CartoonError::GuideDirection { entity });
            }
            (
                *frame,
                RibbonDeformation {
                    controls: control_rows(input.entities, sample.segment as usize),
                    parameter: [
                        sample.parameter,
                        f32::from_bits(u32::MAX),
                        f32::from_bits(u32::MAX),
                        0.0,
                    ],
                },
            )
        };
        let style = match input.styles.get(guide) {
            Some(&style) => style,
            None => SecondaryStructure::Unknown,
        };
        let (_, depth) = profile_extents_for_guide(
            build.params,
            style,
            0.0,
            input.styles,
            guide,
            input.guides.get(guide).copied(),
        );
        append_triangle(sample.position, frame, entity, recipe, depth, build)?;
    }
    Ok(())
}

fn singleton(
    entity: u32,
    guide: Option<GuideKind>,
    structure: Option<&molframe::Structure>,
) -> Result<(TransportFrame, RibbonDeformation), CartoonError> {
    let failure = || CartoonError::GuideDirection { entity };
    let Some((EntityKind::Atom, row)) = EntityId(entity).unpack() else {
        return Err(failure());
    };
    let atom = structure
        .and_then(|structure| {
            structure
                .atoms()
                .get(molframe::AtomIndex::new(row).as_usize())
        })
        .ok_or_else(failure)?;
    let residue = atom.residue().ok_or_else(failure)?;
    let (start, end) = if guide == Some(GuideKind::SugarCarbon) {
        ("C5'", "C3'")
    } else {
        ("N", "C")
    };
    let start = residue.atom(start).ok_or_else(failure)?;
    let end = residue.atom(end).ok_or_else(failure)?;
    let start_position = Vec3::from(start.position().ok_or_else(failure)?);
    let end_position = Vec3::from(end.position().ok_or_else(failure)?);
    let tangent = (end_position - start_position)
        .try_normalize()
        .ok_or_else(failure)?;
    let seed = if tangent.z.abs() < 0.9 {
        Vec3::Z
    } else {
        Vec3::X
    };
    let normal = tangent.cross(seed).normalize();
    let frame = TransportFrame {
        tangent,
        normal,
        binormal: tangent.cross(normal),
    };
    Ok((
        frame,
        RibbonDeformation {
            controls: [start.index().get(), row, end.index().get(), row],
            parameter: [
                0.0,
                f32::from_bits(u32::MAX),
                f32::from_bits(u32::MAX),
                f32::from_bits(ATOM_ANCHOR),
            ],
        },
    ))
}

fn append_triangle(
    guide: Vec3,
    frame: TransportFrame,
    entity: u32,
    recipe: RibbonDeformation,
    depth: f32,
    build: &mut DirectionBuild<'_>,
) -> Result<(), CartoonError> {
    let base = super::draw_limits::vertex_index(build.vertices.len())?;
    super::draw_limits::check_glyph(build.vertices.len(), build.indices.len())?;
    let width = build.params.width.abs() * WIDTH_FACTOR;
    let centre = guide + frame.binormal * (depth + FACE_CLEARANCE);
    for (forward, lateral) in [
        (TIP_FRACTION, 0.0),
        (TIP_FRACTION - 1.0, 0.5),
        (TIP_FRACTION - 1.0, -0.5),
    ] {
        build.vertices.push(RibbonVertex {
            position: (centre
                + frame.tangent * (forward * width)
                + frame.normal * (lateral * width))
                .to_array(),
            entity_id: entity,
            normal: frame.binormal.to_array(),
            color: build.params.color,
        });
        build.deformations.push(recipe);
    }
    build.indices.extend_from_slice(&[base, base + 1, base + 2]);
    Ok(())
}

#[cfg(test)]
#[path = "directions_tests.rs"]
mod tests;
