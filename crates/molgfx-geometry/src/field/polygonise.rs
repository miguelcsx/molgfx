//! Shared tetrahedral polygonisation and edge-based vertex reuse.

use super::grid::{Grid, Node};
use super::{BoundaryMesh, BoundaryVertex, FieldError};
use molgfx_math::Vec3;
use num_traits::ToPrimitive as _;
use std::collections::HashMap;

pub(super) struct Cell {
    pub(super) nodes: [Node; 8],
    pub(super) values: [f32; 8],
    pub(super) level: f32,
    pub(super) label: u32,
}

#[derive(Default)]
pub(super) struct Builder {
    pub(super) mesh: BoundaryMesh,
    // One extraction owns this cache. Exact label and source-node endpoints
    // distinguish boundaries; adjacent cells share vertices until it is dropped.
    edges: HashMap<(u32, u64, u64), u32>,
}

impl Builder {
    pub(super) fn cell(&mut self, grid: &Grid<'_>, cell: &Cell) -> Result<(), FieldError> {
        if cell.values.iter().all(|value| *value >= cell.level)
            || cell.values.iter().all(|value| *value < cell.level)
        {
            return Ok(());
        }
        // All six permutations walk the same cube diagonal. Shared face
        // diagonals agree between neighbours, preventing cracks.
        for axes in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let tetrahedron = [0, 1 << axes[0], (1 << axes[0]) | (1 << axes[1]), 7];
            self.tetrahedron(grid, cell, tetrahedron)?;
        }
        Ok(())
    }

    fn tetrahedron(
        &mut self,
        grid: &Grid<'_>,
        cell: &Cell,
        tetrahedron: [usize; 4],
    ) -> Result<(), FieldError> {
        let mut inside = [0; 4];
        let mut outside = [0; 4];
        let mut counts = [0; 2];
        let mut centres = [Vec3::ZERO; 2];
        for corner in tetrahedron {
            let side = usize::from(cell.values[corner] < cell.level);
            centres[side] += cell.nodes[corner].point;
            if side == 0 {
                inside[counts[side]] = corner;
            } else {
                outside[counts[side]] = corner;
            }
            counts[side] += 1;
        }
        if counts[0] == 0 || counts[1] == 0 {
            return Ok(());
        }
        let inside_count =
            f32::from(u8::try_from(counts[0]).map_err(|_| FieldError::IndexOverflow)?);
        let outside_count =
            f32::from(u8::try_from(counts[1]).map_err(|_| FieldError::IndexOverflow)?);
        let outward = grid
            .transform
            .transform_vector3(centres[1] / outside_count - centres[0] / inside_count);
        if counts[0] == 2 {
            let vertices = [
                self.vertex(grid, cell, inside[0], outside[0])?,
                self.vertex(grid, cell, inside[0], outside[1])?,
                self.vertex(grid, cell, inside[1], outside[1])?,
                self.vertex(grid, cell, inside[1], outside[0])?,
            ];
            self.triangle([vertices[0], vertices[1], vertices[2]], outward)?;
            self.triangle([vertices[0], vertices[2], vertices[3]], outward)?;
        } else {
            let (single, opposing) = if counts[0] == 1 {
                (inside[0], outside)
            } else {
                (outside[0], inside)
            };
            let vertices = [
                self.vertex(grid, cell, single, opposing[0])?,
                self.vertex(grid, cell, single, opposing[1])?,
                self.vertex(grid, cell, single, opposing[2])?,
            ];
            self.triangle(vertices, outward)?;
        }
        Ok(())
    }

    fn vertex(
        &mut self,
        grid: &Grid<'_>,
        cell: &Cell,
        a: usize,
        b: usize,
    ) -> Result<u32, FieldError> {
        let fraction = ((f64::from(cell.level) - f64::from(cell.values[a]))
            / (f64::from(cell.values[b]) - f64::from(cell.values[a])))
        .to_f32()
        .ok_or(FieldError::NonfiniteGeometry)?;
        let endpoints = if fraction == 0.0 {
            [a; 2]
        } else if fraction.to_bits() == 1.0_f32.to_bits() {
            [b; 2]
        } else {
            [a, b]
        };
        let identities = endpoints.map(|corner| cell.nodes[corner].identity);
        let key = (
            cell.label,
            identities[0].min(identities[1]),
            identities[0].max(identities[1]),
        );
        if let Some(&vertex) = self.edges.get(&key) {
            return Ok(vertex);
        }
        let local = cell.nodes[a].point.lerp(cell.nodes[b].point, fraction);
        let position = grid.transform.transform_point3(local);
        if !position.is_finite() {
            return Err(FieldError::NonfiniteGeometry);
        }
        let normal = grid.normal(local, cell.label)?;
        let index =
            u32::try_from(self.mesh.vertices.len()).map_err(|_| FieldError::IndexOverflow)?;
        self.mesh
            .vertices
            .try_reserve(1)
            .map_err(|_| FieldError::Allocation)?;
        self.edges
            .try_reserve(1)
            .map_err(|_| FieldError::Allocation)?;
        self.mesh.vertices.push(BoundaryVertex {
            position: position.to_array(),
            normal: normal.to_array(),
            label: cell.label,
            padding: 0,
        });
        let _ = self.edges.insert(key, index);
        Ok(index)
    }

    fn triangle(&mut self, mut indices: [u32; 3], outward: Vec3) -> Result<(), FieldError> {
        let mut points = [Vec3::ZERO; 3];
        for (point, index) in points.iter_mut().zip(indices) {
            *point = Vec3::from_array(
                self.mesh.vertices
                    [usize::try_from(index).map_err(|_| FieldError::IndexOverflow)?]
                .position,
            );
        }
        let normal = (points[1] - points[0]).cross(points[2] - points[0]);
        if !normal.is_finite() {
            return Err(FieldError::NonfiniteGeometry);
        }
        if normal == Vec3::ZERO {
            return Ok(());
        }
        if normal.dot(outward) < 0.0 {
            indices.swap(1, 2);
        }
        self.mesh
            .triangles
            .try_reserve(1)
            .map_err(|_| FieldError::Allocation)?;
        self.mesh.triangles.push(indices);
        Ok(())
    }
}
