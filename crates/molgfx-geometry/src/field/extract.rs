//! Complete scalar and categorical boundary extraction.

use super::grid::Grid;
use super::polygonise::{Builder, Cell};
use super::{BoundaryMesh, FieldError};
use molgfx_core::{ScalarVolume, SegmentedVolume};

/// Extracts a scalar boundary with shared vertices and affine outward normals.
///
/// Source values remain borrowed. Traversal is linear in source cells and
/// emitted topology; the temporary edge cache uses memory proportional to
/// emitted vertices. Levels without a crossing return valid empty geometry.
///
/// # Errors
/// Rejects nonfinite levels, unrepresentable geometry and allocation failure.
pub fn extract_isosurface(volume: &ScalarVolume, level: f32) -> Result<BoundaryMesh, FieldError> {
    if !level.is_finite() {
        return Err(FieldError::InvalidLevel);
    }
    let [minimum, maximum] = volume.range();
    if level <= minimum || level > maximum {
        return Ok(BoundaryMesh::default());
    }
    let grid = Grid::scalar(volume)?;
    let mut builder = Builder::default();
    for z in 0..i32::from(grid.dimensions[2]) - 1 {
        for y in 0..i32::from(grid.dimensions[1]) - 1 {
            for x in 0..i32::from(grid.dimensions[0]) - 1 {
                let nodes = grid.cube([x, y, z])?;
                let mut values = [0.0; 8];
                for (value, node) in values.iter_mut().zip(nodes) {
                    *value = grid.value(node.coordinates, 0)?;
                }
                builder.cell(
                    &grid,
                    &Cell {
                        nodes,
                        values,
                        level,
                        label: 0,
                    },
                )?;
            }
        }
    }
    Ok(builder.mesh)
}

/// Extracts closed boundaries of exact integer labels, including zero.
///
/// Each cell considers only its at most eight distinct labels, so work scales
/// with grid size rather than the total category count. Reconstruction filters
/// centre-dominant binary membership, never label identities. The filter
/// preserves every nodal classification, including isolated labels. Exterior nodes have no label,
/// closing categories at the finite source boundary without reserving an ID.
///
/// # Errors
/// Rejects unrepresentable geometry and allocation failure.
pub fn extract_label_surfaces(volume: &SegmentedVolume) -> Result<BoundaryMesh, FieldError> {
    let grid = Grid::labels(volume)?;
    let mut builder = Builder::default();
    for z in -1..i32::from(grid.dimensions[2]) {
        for y in -1..i32::from(grid.dimensions[1]) {
            for x in -1..i32::from(grid.dimensions[0]) {
                let nodes = grid.cube([x, y, z])?;
                let mut labels = [None; 8];
                for (label, node) in labels.iter_mut().zip(nodes) {
                    *label = grid.label(node.coordinates)?;
                }
                for (corner, &candidate) in labels.iter().enumerate() {
                    let Some(label) = candidate else { continue };
                    if labels[..corner].contains(&candidate) {
                        continue;
                    }
                    let mut values = [0.0; 8];
                    for (value, node) in values.iter_mut().zip(nodes) {
                        *value = grid.value(node.coordinates, label)?;
                    }
                    if values.iter().all(|value| *value >= 0.5)
                        || values.iter().all(|value| *value < 0.5)
                    {
                        continue;
                    }
                    // Refine only crossing cells. Shared half-grid identities
                    // keep neighbours watertight without duplicating source data.
                    for corner in 0..8 {
                        let origin = [x, y, z].map(|coordinate| coordinate * 2);
                        let origin = std::array::from_fn(|axis| {
                            origin[axis] + i32::from(corner & (1 << axis) != 0)
                        });
                        let nodes = grid.refined_cube(origin)?;
                        let mut values = [0.0; 8];
                        for (value, node) in values.iter_mut().zip(nodes) {
                            *value = grid.sample(node.point, label)?;
                        }
                        builder.cell(
                            &grid,
                            &Cell {
                                nodes,
                                values,
                                level: 0.5,
                                label,
                            },
                        )?;
                    }
                }
            }
        }
    }
    Ok(builder.mesh)
}
