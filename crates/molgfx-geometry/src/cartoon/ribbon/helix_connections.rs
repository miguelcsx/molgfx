//! Full-run helix fits for adjacent coil anchors, O(guides + vertices).

use super::{RibbonMesh, helix::Run, helix_cells::row};
use crate::PackingError;
use std::collections::{HashMap, HashSet};

pub(super) fn seed(
    mesh: &mut RibbonMesh,
    lookup: &mut HashMap<(u32, u32), usize>,
    runs: &mut Vec<Run>,
    guide_indices: &HashMap<u32, usize>,
) -> Result<(), PackingError> {
    if !mesh.classic {
        return Ok(());
    }
    let selected: HashSet<_> = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.entity_id)
        .collect();
    let fragments = if mesh.traces.parent_ranges.is_empty() {
        &mesh.traces.ranges
    } else {
        &mesh.traces.parent_ranges
    };
    for fragment in fragments {
        let mut start = fragment.points.start;
        while start < fragment.points.end {
            if !mesh.traces.styles[start].is_helix() {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < fragment.points.end && mesh.traces.styles[end].is_helix() {
                end += 1;
            }
            if end - start >= 2
                && mesh.traces.entities[start.saturating_sub(1).max(fragment.points.start)
                    ..(end + 1).min(fragment.points.end)]
                    .iter()
                    .any(|entity| selected.contains(entity))
            {
                let key = (
                    row(mesh.traces.entities[start])?,
                    row(mesh.traces.entities[end - 1])?,
                );
                let destination =
                    super::super::draw_limits::vertex_index(mesh.traces.properties.len())?;
                mesh.traces
                    .properties
                    .resize(mesh.traces.properties.len() + 24, 0.0);
                let mut fit = Run {
                    destination,
                    ..Run::default()
                };
                fit.fill_source_guides(key, guide_indices, &mesh.traces.entities)?;
                lookup.insert(key, runs.len());
                runs.push(fit);
            }
            start = end;
        }
    }
    Ok(())
}

pub(super) fn bind(mesh: &mut RibbonMesh, runs: &[Run]) -> Result<(), PackingError> {
    let mut guides = HashMap::new();
    for fit in runs {
        let count = crate::cartoon::draw_limits::exact_f32(fit.rows.len(), "helix sequence")?;
        let midpoint = (count - 1.0) * 0.5;
        for (index, &row) in fit.rows.iter().enumerate() {
            let sequence =
                crate::cartoon::draw_limits::exact_f32(index, "helix sequence")? - midpoint;
            let sequence = sequence
                + if index == 0 {
                    -0.5
                } else if index + 1 == fit.rows.len() {
                    0.5
                } else {
                    0.0
                };
            guides.insert(row, (fit.destination, sequence));
        }
    }
    for recipe in &mesh.deformations {
        if recipe.parameter[3].to_bits() != 14 {
            continue;
        }
        for &stencil in &recipe.controls {
            let offset = stencil as usize;
            let row = mesh.traces.properties[offset + 1].to_bits();
            if let Some(&(destination, sequence)) = guides.get(&row) {
                mesh.traces.properties[offset] = f32::from_bits(destination);
                mesh.traces.properties[offset + 2] = sequence;
                mesh.traces.properties[offset + 3] = f32::from_bits(3);
            }
        }
    }
    Ok(())
}
