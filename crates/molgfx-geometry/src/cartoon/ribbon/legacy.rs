//! O(guides × steps) double-edge spline sheets and closed border tubes.
//! GPU deformation gathers a fixed number of borrowed guide and plane rows
//! per vertex; no coordinate column or trajectory frame is duplicated.

use super::{RibbonDeformation, RibbonMesh, RibbonParams, RibbonVertex, control_rows};
use crate::{CartoonError, PackingError};
use molgfx_core::AtomSelection;
use molgfx_math::{Vec3, catmull_rom_position, catmull_rom_tangent};

const SIDES: u8 = 16;
const SHELL_LANES: u8 = 2 + 2 * SIDES;
const CAP_START: u32 = 256;
const CAP_END: u32 = 512;

#[cfg(test)]
#[path = "legacy_tests.rs"]
mod tests;

/// The local controls of two independently evaluated border splines.
struct Edges {
    left: [Vec3; 4],
    right: [Vec3; 4],
    radius: f32,
    controls: [u32; 4],
    offset: u32,
    segment: usize,
    visibility: Option<[f32; 2]>,
}

impl RibbonMesh {
    /// Builds a flat sheet between independently splined edges, with optional
    /// circular edge tubes. Width cues follow peptide oxygens where available.
    /// Every vertex keeps live source anchors, including the closed tube ends.
    ///
    /// # Errors
    /// Rejects invalid dimensions, missing source rows or GPU index overflow.
    pub fn generate_legacy_ribbons(
        &mut self,
        structure: &molframe::Structure,
        selection: &AtomSelection,
        max_gap: f32,
        params: RibbonParams,
        edge_radius: f32,
    ) -> Result<(), CartoonError> {
        if !params.width.is_finite()
            || params.width <= 0.0
            || !edge_radius.is_finite()
            || edge_radius < 0.0
        {
            return Err(PackingError::Topology {
                operation: "ribbon width and edge radius",
            }
            .into());
        }
        self.clear();
        super::super::traces::extract_contextual_polymer_traces(
            structure,
            selection,
            &[],
            max_gap,
            &mut self.traces,
        )?;
        self.traces.properties.clear();
        let result = self
            .append_legacy_traces(params, edge_radius)
            .and_then(|()| self.append_sugar_faces(structure, selection, params.color));
        if result.is_err() {
            self.clear();
        }
        result.map_err(CartoonError::from)
    }

    fn append_legacy_traces(
        &mut self,
        params: RibbonParams,
        radius: f32,
    ) -> Result<(), PackingError> {
        let steps = params.max_steps.max(2);
        for range_index in 0..self.traces.ranges.len() {
            let range = self.traces.ranges[range_index].points.clone();
            self.demand.clear();
            let mut previous_width = Vec3::ZERO;
            for index in range.clone() {
                let previous = index.saturating_sub(1).max(range.start);
                let next = (index + 1).min(range.end - 1);
                let width = width_direction(
                    self.traces.points[previous],
                    self.traces.points[index],
                    self.traces.points[next],
                    self.traces.normals[index],
                );
                let sign = if width.dot(previous_width) < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                self.demand.push(sign);
                previous_width = width * sign;
            }
            for segment in 0..range.len() - 1 {
                let mut edges = self.legacy_edges(range.clone(), segment, params.width, radius)?;
                let segment_parameter =
                    crate::cartoon::draw_limits::exact_f32(segment, "ribbon segment parameter")?;
                let visibility = match self.traces.visible_parameters.get(range_index) {
                    Some(&range) => range,
                    None => [
                        0.0,
                        crate::cartoon::draw_limits::exact_f32(
                            range.len() - 1,
                            "ribbon span parameter",
                        )?,
                    ],
                };
                edges.visibility = Some(visibility);
                let start = (visibility[0] - segment_parameter).max(0.0);
                let end = (visibility[1] - segment_parameter).min(1.0);
                if start >= end {
                    continue;
                }
                let base = super::super::draw_limits::vertex_index(self.vertices.len())?;
                let lanes = if radius > 0.0 { SHELL_LANES } else { 2 };
                check_capacity(
                    self,
                    (usize::from(steps) + 1) * usize::from(lanes) + 68,
                    usize::from(steps) * 198 + 192,
                )?;
                for step in 0..=steps {
                    let t = start + (end - start) * f32::from(step) / f32::from(steps);
                    for lane in 0..u32::from(lanes) {
                        self.legacy_vertex(&edges, t, lane, params.color);
                    }
                }
                append_shell(&mut self.indices, base, steps, u32::from(lanes));
                if radius > 0.0 && (segment_parameter + start).to_bits() == visibility[0].to_bits()
                {
                    self.legacy_cap(&edges, start, CAP_START, params.color)?;
                }
                if radius > 0.0 && (segment_parameter + end).to_bits() == visibility[1].to_bits() {
                    self.legacy_cap(&edges, end, CAP_END, params.color)?;
                }
            }
        }
        Ok(())
    }

    fn legacy_edges(
        &mut self,
        range: std::ops::Range<usize>,
        segment: usize,
        width: f32,
        radius: f32,
    ) -> Result<Edges, PackingError> {
        let points = &self.traces.points[range.clone()];
        let entities = &self.traces.entities[range.clone()];
        let controls = control_rows(entities, segment);
        let guide_indices = [
            segment.saturating_sub(1),
            segment,
            segment + 1,
            (segment + 2).min(points.len() - 1),
        ];
        let offset = super::super::draw_limits::vertex_index(self.traces.properties.len())?;
        let mut edges = Edges {
            left: [Vec3::ZERO; 4],
            right: [Vec3::ZERO; 4],
            radius,
            controls,
            offset,
            segment,
            visibility: None,
        };
        // Explicit adjacent rows preserve the same guide cue in every
        // interval, including duplicated endpoint controls.
        for index in guide_indices {
            for adjacent in [index.saturating_sub(1), (index + 1).min(points.len() - 1)] {
                self.traces
                    .properties
                    .push(f32::from_bits(entities[adjacent] & 0x0fff_ffff));
            }
        }
        for index in guide_indices {
            self.traces
                .properties
                .push(f32::from_bits(self.traces.plane_rows[range.start + index]));
        }
        for index in guide_indices {
            self.traces.properties.push(self.demand[index]);
        }
        self.traces.properties.extend([width * 0.5, radius]);
        for (lane, index) in guide_indices.into_iter().enumerate() {
            let direction = width_direction(
                points[index.saturating_sub(1)],
                points[index],
                points[(index + 1).min(points.len() - 1)],
                self.traces.normals[range.start + index],
            ) * self.demand[index]
                * width
                * 0.5;
            edges.left[lane] = points[index] - direction;
            edges.right[lane] = points[index] + direction;
        }
        Ok(edges)
    }

    fn legacy_vertex(&mut self, edges: &Edges, t: f32, lane: u32, color: molgfx_math::Rgba8) {
        let (position, normal) = edges.vertex(t, lane);
        let owner = super::super::selection_span::entity_index(edges.segment, t, edges.visibility);
        let entity_row = if owner == edges.segment {
            edges.controls[1]
        } else {
            edges.controls[2]
        };
        self.vertices.push(RibbonVertex {
            position: position.to_array(),
            normal: normal.to_array(),
            entity_id: entity_row,
            color,
        });
        self.deformations.push(RibbonDeformation {
            controls: edges.controls,
            parameter: [
                t,
                f32::from_bits(edges.offset),
                f32::from_bits(lane),
                f32::from_bits(9),
            ],
        });
    }

    fn legacy_cap(
        &mut self,
        edges: &Edges,
        t: f32,
        mode: u32,
        color: molgfx_math::Rgba8,
    ) -> Result<(), PackingError> {
        for side in 0..2u32 {
            let center = super::super::draw_limits::vertex_index(self.vertices.len())?;
            self.legacy_vertex(edges, t, (34 + side) | mode, color);
            for lane in 0..u32::from(SIDES) {
                self.legacy_vertex(edges, t, (2 + side * u32::from(SIDES) + lane) | mode, color);
            }
            for lane in 0..u32::from(SIDES) {
                let a = center + 1 + lane;
                let b = center + 1 + (lane + 1) % u32::from(SIDES);
                self.indices.extend(if mode == CAP_START {
                    [center, b, a]
                } else {
                    [center, a, b]
                });
            }
        }
        Ok(())
    }
}

impl Edges {
    fn vertex(&self, t: f32, encoded: u32) -> (Vec3, Vec3) {
        let lane = encoded & 255;
        let left = catmull_rom_position(self.left, t);
        let right = catmull_rom_position(self.right, t);
        let across = unit(right - left, Vec3::X);
        let left_tangent = catmull_rom_tangent(self.left, t);
        let right_tangent = catmull_rom_tangent(self.right, t);
        if lane < 2 {
            return (
                if lane == 0 { left } else { right },
                unit((left_tangent + right_tangent).cross(across), Vec3::Z),
            );
        }
        let side = if lane >= 34 {
            lane - 34
        } else {
            (lane - 2) / u32::from(SIDES)
        };
        let tangent = unit(
            if side == 0 {
                left_tangent
            } else {
                right_tangent
            },
            Vec3::Z,
        );
        let center = if side == 0 { left } else { right };
        let radial = unit(across - tangent * across.dot(tangent), Vec3::X);
        let binormal = tangent.cross(radial);
        let (radial_weight, binormal_weight, _) = super::super::cross_section::cross_section(
            molgfx_core::CartoonProfile::Elliptical,
            (lane - 2) as usize,
            1.0,
            1.0,
        );
        let normal = radial * radial_weight + binormal * binormal_weight;
        let position = if lane >= 34 {
            center
        } else {
            center + normal * self.radius
        };
        let normal = match encoded & !255 {
            CAP_START => -tangent,
            CAP_END => tangent,
            _ => normal,
        };
        (position, normal)
    }
}

fn width_direction(previous: Vec3, center: Vec3, next: Vec3, cue: Vec3) -> Vec3 {
    let tangent = unit(next - previous, Vec3::Z);
    let projected = cue - tangent * cue.dot(tangent);
    if let Some(width) = projected.try_normalize() {
        return width;
    }
    let bend = previous + next - center * 2.0;
    if let Some(width) = (bend - tangent * bend.dot(tangent)).try_normalize() {
        return width;
    }
    let seed = if tangent.x.abs() <= tangent.y.abs() && tangent.x.abs() <= tangent.z.abs() {
        Vec3::X
    } else if tangent.y.abs() <= tangent.z.abs() {
        Vec3::Y
    } else {
        Vec3::Z
    };
    unit(seed - tangent * seed.dot(tangent), Vec3::X)
}

fn unit(value: Vec3, fallback: Vec3) -> Vec3 {
    match value.try_normalize() {
        Some(value) => value,
        None => fallback,
    }
}

fn append_shell(indices: &mut Vec<u32>, base: u32, steps: u8, lanes: u32) {
    for step in 0..u32::from(steps) {
        let a = base + step * lanes;
        let b = a + lanes;
        indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        if lanes == 2 {
            continue;
        }
        for side in 0..2 {
            for lane in 0..u32::from(SIDES) {
                let x = 2 + side * u32::from(SIDES) + lane;
                let y = 2 + side * u32::from(SIDES) + (lane + 1) % u32::from(SIDES);
                indices.extend([a + x, a + y, b + x, a + y, b + y, b + x]);
            }
        }
    }
}

fn check_capacity(mesh: &RibbonMesh, vertices: usize, indices: usize) -> Result<(), PackingError> {
    super::super::draw_limits::vertex_index(mesh.vertices.len().saturating_add(vertices))?;
    u32::try_from(mesh.indices.len().saturating_add(indices)).map_err(|_| {
        PackingError::IndexOverflow {
            resource: "ribbon draw indices",
            index: u64::MAX,
        }
    })?;
    Ok(())
}
