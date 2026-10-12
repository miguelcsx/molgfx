//! Closed residue-owned circular helix columns over shared live fits, O(guides + vertices).

use super::{PROFILE_SIDES, RibbonDeformation, RibbonMesh, RibbonParams, RibbonVertex};
use crate::PackingError;
use molgfx_core::{EntityId, EntityKind, SecondaryStructure};
use molgfx_math::Vec3;
use num_traits::ToPrimitive;
use std::{collections::HashSet, ops::Range};

impl RibbonMesh {
    pub(super) fn append_classic_helices(
        &mut self,
        points: &[Vec3],
        entities: &[u32],
        styles: &[SecondaryStructure],
        fragments: &[Range<usize>],
        params: RibbonParams,
        selected: &HashSet<u32>,
    ) -> Result<(), PackingError> {
        let (runs, helices) =
            super::strands::selected_runs(selected, entities, styles, fragments, true);
        super::strand_connections::retain_coils(self, selected, &helices, &runs, entities)?;
        for run in runs {
            let (center, slope) = super::super::rocket::helix_fit(&points[run.clone()]).ok_or(
                PackingError::GeometryBudget {
                    resource: "helix fit",
                },
            )?;
            let tangent = match slope.try_normalize() {
                Some(axis) => axis,
                None => Vec3::X,
            };
            let seed = if tangent.x.abs() < 0.9 {
                Vec3::X
            } else {
                Vec3::Y
            };
            let width = tangent.cross(seed).normalize();
            let height = tangent.cross(width);
            let count = run.len().to_f32().ok_or(PackingError::GeometryBudget {
                resource: "helix sequence",
            })?;
            for (index, &entity) in entities[run.clone()].iter().enumerate() {
                if !selected.contains(&entity) {
                    continue;
                }
                let sequence = index.to_f32().ok_or(PackingError::GeometryBudget {
                    resource: "helix sequence",
                })? - (count - 1.0) * 0.5;
                let cell = Cell {
                    center: center + slope * sequence,
                    slope,
                    tangent,
                    width,
                    height,
                    radius: params.helix_radius,
                    entity,
                    controls: [
                        row(entity)?,
                        row(entity)?,
                        row(entities[run.start])?,
                        row(entities[run.end - 1])?,
                    ],
                    color: params.color,
                };
                append_cell(self, cell)?;
                if index == 0 || !selected.contains(&entities[run.start + index - 1]) {
                    append_cap(self, cell, -0.5)?;
                }
                if index + 1 == run.len() || !selected.contains(&entities[run.start + index + 1]) {
                    append_cap(self, cell, 0.5)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Cell {
    center: Vec3,
    slope: Vec3,
    tangent: Vec3,
    width: Vec3,
    height: Vec3,
    radius: f32,
    entity: u32,
    controls: [u32; 4],
    color: molgfx_math::Rgba8,
}

fn append_cell(mesh: &mut RibbonMesh, cell: Cell) -> Result<(), PackingError> {
    reserve(mesh, PROFILE_SIDES * 2, PROFILE_SIDES * 6)?;
    let base = super::super::draw_limits::vertex_index(mesh.vertices.len())?;
    for parameter in [-0.5, 0.5] {
        for side in 0..PROFILE_SIDES {
            let normal = radial(cell, side)?;
            vertex(mesh, cell, parameter, normal * cell.radius, normal);
        }
    }
    let stride = super::super::draw_limits::vertex_index(PROFILE_SIDES)?;
    for side in 0..stride {
        let next = (side + 1) % stride;
        mesh.indices.extend([
            base + side,
            base + stride + next,
            base + stride + side,
            base + side,
            base + next,
            base + stride + next,
        ]);
    }
    Ok(())
}

fn append_cap(mesh: &mut RibbonMesh, cell: Cell, parameter: f32) -> Result<(), PackingError> {
    reserve(mesh, PROFILE_SIDES + 1, PROFILE_SIDES * 3)?;
    let base = super::super::draw_limits::vertex_index(mesh.vertices.len())?;
    let normal = cell.tangent * parameter.signum();
    vertex(mesh, cell, parameter, Vec3::ZERO, normal);
    for side in 0..PROFILE_SIDES {
        vertex(
            mesh,
            cell,
            parameter,
            radial(cell, side)? * cell.radius,
            normal,
        );
    }
    let count = super::super::draw_limits::vertex_index(PROFILE_SIDES)?;
    for side in 0..count {
        let left = base + 1 + side;
        let right = base + 1 + (side + 1) % count;
        if parameter > 0.0 {
            mesh.indices.extend([base, left, right]);
        } else {
            mesh.indices.extend([base, right, left]);
        }
    }
    Ok(())
}

fn vertex(mesh: &mut RibbonMesh, cell: Cell, parameter: f32, offset: Vec3, normal: Vec3) {
    mesh.vertices.push(RibbonVertex {
        position: (cell.center + cell.slope * parameter + offset).to_array(),
        normal: normal.to_array(),
        entity_id: cell.entity,
        color: cell.color,
    });
    mesh.deformations.push(RibbonDeformation {
        controls: cell.controls,
        parameter: [parameter, 0.0, 0.0, f32::from_bits(6)],
    });
}

fn radial(cell: Cell, facet: usize) -> Result<Vec3, PackingError> {
    let amount = facet.to_f32().ok_or(PackingError::GeometryBudget {
        resource: "helix sides",
    })?;
    let count = PROFILE_SIDES.to_f32().ok_or(PackingError::GeometryBudget {
        resource: "helix sides",
    })?;
    let (sine, cosine) = (amount * std::f32::consts::TAU / count).sin_cos();
    Ok(cell.width * cosine + cell.height * sine)
}

fn reserve(mesh: &RibbonMesh, vertices: usize, indices: usize) -> Result<(), PackingError> {
    if mesh.vertices.len().saturating_add(vertices) > molgfx_core::MAX_MESH_VERTICES {
        return Err(PackingError::GeometryBudget {
            resource: "helix vertices",
        });
    }
    super::super::draw_limits::vertex_index(mesh.indices.len().saturating_add(indices))?;
    Ok(())
}

fn row(entity: u32) -> Result<u32, PackingError> {
    match EntityId(entity).unpack() {
        Some((EntityKind::Atom, row)) => Ok(row),
        _ => Err(PackingError::GeometryBudget {
            resource: "helix atom provenance",
        }),
    }
}
