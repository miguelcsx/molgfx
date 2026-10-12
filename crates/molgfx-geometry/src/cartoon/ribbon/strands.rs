//! Closed residue-owned beta blocks over shared live frame jobs, in O(guides + vertices).

use super::{RibbonDeformation, RibbonMesh, RibbonParams, RibbonVertex, strand_frames};
use crate::PackingError;
use molgfx_core::{EntityId, EntityKind, SecondaryStructure};
use molgfx_math::Vec3;
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

pub(super) const STRAND_JOB: u32 = 1 << 31;

#[derive(Clone, Debug)]
pub(super) struct StrandFit {
    pub rows: Vec<u32>,
    pub destination: u32,
}

impl RibbonMesh {
    pub(super) fn append_classic_strands(
        &mut self,
        points: &[Vec3],
        entities: &[u32],
        styles: &[SecondaryStructure],
        fragments: &[Range<usize>],
        params: RibbonParams,
        selected: &HashSet<u32>,
    ) -> Result<(), PackingError> {
        let (runs, beta) = selected_runs(selected, entities, styles, fragments, false);
        if runs.is_empty() {
            return Ok(());
        }
        super::strand_connections::retain_coils(self, selected, &beta, &runs, entities)?;
        for run in runs {
            let frames = strand_frames::frames(&points[run.clone()]);
            let destination =
                super::super::draw_limits::vertex_index(self.traces.properties.len())?;
            let count = frames
                .len()
                .checked_mul(36)
                .ok_or(PackingError::GeometryBudget {
                    resource: "strand frame cache",
                })?;
            self.traces
                .properties
                .resize(self.traces.properties.len() + count, 0.0);
            let rows = entities[run.clone()]
                .iter()
                .map(|&entity| match EntityId(entity).unpack() {
                    Some((EntityKind::Atom, row)) => Ok(row),
                    _ => Err(PackingError::GeometryBudget {
                        resource: "strand atom provenance",
                    }),
                })
                .collect::<Result<Vec<_>, _>>()?;
            for cell in 0..run.len() {
                let entity = entities[run.start + cell];
                if !selected.contains(&entity) {
                    continue;
                }
                let arrow = cell + 1 == run.len() && params.arrow_factor > 0.0;
                let body = Body {
                    destination,
                    count: run.len() + 1,
                    cell,
                    entity,
                    width: params.width.abs() * 0.5,
                    thickness: params.thickness.abs() * 0.5,
                    arrow,
                    arrow_factor: params.arrow_factor,
                    color: params.color,
                };
                append_body(self, &frames, body)?;
                if cell == 0 || !selected.contains(&entities[run.start + cell - 1]) || arrow {
                    append_cap(self, &frames, body, false)?;
                }
                if cell + 1 == run.len()
                    || !selected.contains(&entities[run.start + cell + 1])
                    || (cell + 2 == run.len() && params.arrow_factor > 0.0)
                {
                    append_cap(self, &frames, body, true)?;
                }
            }
            self.strand_fits.push(StrandFit { rows, destination });
        }
        super::strand_connections::bind_boundaries(self)
    }

    pub(super) fn append_structure_strands(
        &mut self,
        params: RibbonParams,
    ) -> Result<(), PackingError> {
        let selected: HashSet<_> = self
            .vertices
            .iter()
            .map(|vertex| vertex.entity_id)
            .collect();
        let mut traces = std::mem::take(&mut self.traces);
        let fragments: Vec<_> = traces
            .parent_ranges
            .iter()
            .map(|range| range.points.clone())
            .collect();
        // Property output must stay in the same retained column after borrowing source traces.
        self.traces.properties = std::mem::take(&mut traces.properties);
        let result = self
            .append_classic_strands(
                &traces.points,
                &traces.entities,
                &traces.styles,
                &fragments,
                params,
                &selected,
            )
            .and_then(|()| {
                self.append_classic_helices(
                    &traces.points,
                    &traces.entities,
                    &traces.styles,
                    &fragments,
                    params,
                    &selected,
                )
            });
        traces.properties = std::mem::take(&mut self.traces.properties);
        self.traces = traces;
        result
    }
}

#[derive(Clone, Copy)]
struct Body {
    destination: u32,
    count: usize,
    cell: usize,
    entity: u32,
    width: f32,
    thickness: f32,
    arrow: bool,
    arrow_factor: f32,
    color: molgfx_math::Rgba8,
}

fn append_body(
    mesh: &mut RibbonMesh,
    frames: &[strand_frames::StrandFrame],
    body: Body,
) -> Result<(), PackingError> {
    const CORNERS: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    for (minimum, maximum) in [(0.0, 0.5), (0.5, 1.0)] {
        for edge in 0..4 {
            let next = (edge + 1) % 4;
            let face = [3, 0, 2, 1][edge];
            reserve_quad(mesh)?;
            let base = super::super::draw_limits::vertex_index(mesh.vertices.len())?;
            for (parameter, corner) in [
                (minimum, edge),
                (maximum, edge),
                (maximum, next),
                (minimum, next),
            ] {
                append_vertex(mesh, frames, body, parameter, CORNERS[corner], face)?;
            }
            mesh.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    Ok(())
}

fn append_cap(
    mesh: &mut RibbonMesh,
    frames: &[strand_frames::StrandFrame],
    body: Body,
    end: bool,
) -> Result<(), PackingError> {
    reserve_quad(mesh)?;
    let base = super::super::draw_limits::vertex_index(mesh.vertices.len())?;
    let parameter = if end { 1.0 } else { 0.0 };
    for corner in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
        append_vertex(
            mesh,
            frames,
            body,
            parameter,
            corner,
            if end { 5 } else { 4 },
        )?;
    }
    if end {
        mesh.indices
            .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    } else {
        mesh.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Ok(())
}

fn reserve_quad(mesh: &RibbonMesh) -> Result<(), PackingError> {
    if mesh.vertices.len().saturating_add(4) > molgfx_core::MAX_MESH_VERTICES {
        return Err(PackingError::GeometryBudget {
            resource: "discrete strand vertices",
        });
    }
    super::super::draw_limits::vertex_index(mesh.indices.len().saturating_add(6))?;
    Ok(())
}

pub(super) fn compact_vertices(mesh: &mut RibbonMesh) -> Result<(), PackingError> {
    let mut used = vec![u32::MAX; mesh.vertices.len()];
    for &index in &mesh.indices {
        used[index as usize] = 0;
    }
    let mut written = 0;
    for (index, mapped) in used.iter_mut().enumerate() {
        if *mapped == u32::MAX {
            continue;
        }
        *mapped = super::super::draw_limits::vertex_index(written)?;
        mesh.vertices[written] = mesh.vertices[index];
        mesh.deformations[written] = mesh.deformations[index];
        written += 1;
    }
    mesh.vertices.truncate(written);
    mesh.deformations.truncate(written);
    for index in &mut mesh.indices {
        *index = used[*index as usize];
    }
    Ok(())
}

fn append_vertex(
    mesh: &mut RibbonMesh,
    frames: &[strand_frames::StrandFrame],
    body: Body,
    parameter: f32,
    corner: [f32; 2],
    face: u32,
) -> Result<(), PackingError> {
    let index = super::super::draw_limits::vertex_index(body.cell)?;
    let count = super::super::draw_limits::vertex_index(body.count)?;
    let frame = strand_frames::sample(frames, body.cell, parameter);
    let full_width = body.width * corner[0] * if body.arrow { body.arrow_factor } else { 1.0 };
    let width = full_width * if body.arrow { 1.0 - parameter } else { 1.0 };
    let height = body.thickness * corner[1];
    let span = frames[body.cell + 1].center - frames[body.cell].center;
    let tangent = match span.try_normalize() {
        Some(axis) => axis,
        None => Vec3::X,
    };
    let taper = if body.arrow && span.length_squared() > 1e-12 {
        tangent * (full_width.abs() / span.length())
    } else {
        Vec3::ZERO
    };
    let normal = match face {
        0 => (frame.width + taper).normalize(),
        1 => (-frame.width + taper).normalize(),
        2 => frame.height,
        3 => -frame.height,
        4 => -tangent,
        _ => tangent,
    };
    mesh.vertices.push(RibbonVertex {
        position: (frame.center + frame.width * width + frame.height * height).to_array(),
        entity_id: body.entity,
        normal: normal.to_array(),
        color: body.color,
    });
    mesh.deformations.push(RibbonDeformation {
        controls: [
            body.destination,
            count,
            index,
            face | (u32::from(body.arrow) << 3),
        ],
        parameter: [parameter, full_width, height, f32::from_bits(15)],
    });
    Ok(())
}

pub(super) fn selected_runs(
    selected: &HashSet<u32>,
    entities: &[u32],
    styles: &[SecondaryStructure],
    fragments: &[Range<usize>],
    helices: bool,
) -> (Vec<Range<usize>>, HashMap<u32, usize>) {
    let mut runs = Vec::new();
    let mut beta = HashMap::new();
    for fragment in fragments {
        let mut first = fragment.start;
        while first < fragment.end {
            let matches = |style: &SecondaryStructure| {
                if helices {
                    style.is_helix()
                } else {
                    style.is_sheet_like()
                }
            };
            if !styles.get(first).is_some_and(matches) {
                first += 1;
                continue;
            }
            let mut end = first + 1;
            while end < fragment.end && styles.get(end).is_some_and(matches) {
                end += 1;
            }
            if end - first >= (if helices { 2 } else { 3 })
                && entities
                    [first.saturating_sub(1).max(fragment.start)..(end + 1).min(fragment.end)]
                    .iter()
                    .any(|entity| selected.contains(entity))
            {
                for &entity in &entities[first..end] {
                    beta.insert(entity, runs.len());
                }
                runs.push(first..end);
            }
            first = end;
        }
    }
    (runs, beta)
}

#[cfg(test)]
#[path = "strands_tests.rs"]
mod tests;
