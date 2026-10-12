//! Connects retained coil sweeps to discrete beta boundaries in O(vertices + guides).

use super::{RibbonDeformation, RibbonMesh};
use crate::PackingError;
use molgfx_core::{EntityId, EntityKind};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

pub(super) fn retain_coils(
    mesh: &mut RibbonMesh,
    selected: &HashSet<u32>,
    beta: &HashMap<u32, usize>,
    runs: &[Range<usize>],
    entities: &[u32],
) -> Result<(), PackingError> {
    let mut interior = Vec::with_capacity(mesh.vertices.len());
    for (vertex, recipe) in mesh.vertices.iter_mut().zip(&mesh.deformations) {
        let Some((left, right)) = owners(recipe, &mesh.traces.properties) else {
            interior.push(false);
            continue;
        };
        let left_run = beta.get(&left).copied();
        let right_run = beta.get(&right).copied();
        interior.push(left_run.is_some() && left_run == right_run);
        match (left_run, right_run) {
            (Some(_), None) => vertex.entity_id = right,
            (None, Some(_)) => vertex.entity_id = left,
            (Some(index), Some(_)) => {
                let run = &runs[index];
                if left == entities[run.start] && recipe.parameter[0].to_bits() == 0 {
                    if let Some(&owner) = run
                        .start
                        .checked_sub(1)
                        .and_then(|index| entities.get(index))
                    {
                        vertex.entity_id = owner;
                    }
                } else if right == entities[run.end - 1]
                    && recipe.parameter[0].to_bits() == 1.0f32.to_bits()
                    && let Some(&owner) = entities.get(run.end)
                {
                    vertex.entity_id = owner;
                }
            }
            _ => {}
        }
    }
    let mut written = 0;
    for offset in (0..mesh.indices.len()).step_by(3) {
        let triangle = [
            mesh.indices[offset],
            mesh.indices[offset + 1],
            mesh.indices[offset + 2],
        ];
        if triangle.iter().all(|&index| interior[index as usize])
            || triangle
                .iter()
                .any(|&index| !selected.contains(&mesh.vertices[index as usize].entity_id))
        {
            continue;
        }
        mesh.indices[written..written + 3].copy_from_slice(&triangle);
        written += 3;
    }
    mesh.indices.truncate(written);
    super::strands::compact_vertices(mesh)
}

pub(super) fn bind_boundaries(mesh: &mut RibbonMesh) -> Result<(), PackingError> {
    let mut boundaries = HashMap::with_capacity(mesh.strand_fits.len() * 2);
    for fit in &mesh.strand_fits {
        let stride = super::super::draw_limits::vertex_index((fit.rows.len() + 1) * 12)?;
        let end = super::super::draw_limits::vertex_index(fit.rows.len() * 12)?;
        if let (Some(&first), Some(&last)) = (fit.rows.first(), fit.rows.last()) {
            boundaries.insert(first, (fit.destination, stride));
            boundaries.insert(last, (fit.destination + end, stride));
        }
    }
    // Several selected spans can carry their own stencil for the same original guide.
    for recipe in &mesh.deformations {
        if recipe.parameter[3].to_bits() != 14 {
            continue;
        }
        for &stencil in &recipe.controls {
            let offset = stencil as usize;
            let row = mesh.traces.properties[offset + 1].to_bits();
            if let Some(&(destination, stride)) = boundaries.get(&row) {
                mesh.traces.properties[offset] = f32::from_bits(destination);
                mesh.traces.properties[offset + 2] = f32::from_bits(stride);
                mesh.traces.properties[offset + 3] = f32::from_bits(2);
            }
        }
    }
    Ok(())
}

fn owners(recipe: &RibbonDeformation, properties: &[f32]) -> Option<(u32, u32)> {
    if recipe.parameter[3].to_bits() == 6 {
        return Some((
            EntityId::pack(EntityKind::Atom, u64::from(recipe.controls[0]))
                .ok()?
                .0,
            EntityId::pack(EntityKind::Atom, u64::from(recipe.controls[1]))
                .ok()?
                .0,
        ));
    }
    if recipe.parameter[3].to_bits() != 14 {
        return None;
    }
    let left = properties.get(recipe.controls[1] as usize + 1)?.to_bits();
    let right = properties.get(recipe.controls[2] as usize + 1)?.to_bits();
    Some((
        EntityId::pack(EntityKind::Atom, u64::from(left)).ok()?.0,
        EntityId::pack(EntityKind::Atom, u64::from(right)).ok()?.0,
    ))
}
