//! Bounded quadratic surfaces lowered to the resident indexed-mesh path.

use crate::{CoreError, Material, Mesh, MeshTopology, MeshVertex, StructureHandle};
use molgfx_math::{Aabb, Rgba8, Vec3};

#[cfg(test)]
#[path = "quadric_tests.rs"]
mod tests;

const MAX_CELLS_PER_AXIS: u16 = 32;
const TETRAHEDRA: [[usize; 4]; 6] = [
    [0, 1, 3, 7],
    [0, 3, 2, 7],
    [0, 2, 6, 7],
    [0, 6, 4, 7],
    [0, 4, 5, 7],
    [0, 5, 1, 7],
];
const EDGES: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

/// A quadratic zero-set clipped to finite model-space bounds.
///
/// Coefficients are ordered as `xx, yy, zz, xy, xz, yz, x, y, z, constant`.
/// The finite cell grid makes open surfaces such as paraboloids and
/// hyperboloids safe to author and cull.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Quadric {
    coefficients: [f32; 10],
    bounds: Aabb,
    cells: [u16; 3],
}

impl Quadric {
    /// Creates a finite, bounded quadratic surface sampling contract.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for non-finite coefficients or
    /// bounds, empty bounds, or a cell count outside `1..=32` on any axis.
    pub fn new(coefficients: [f32; 10], bounds: Aabb, cells: [u16; 3]) -> Result<Self, CoreError> {
        if coefficients.iter().any(|value| !value.is_finite())
            || bounds.is_empty()
            || !bounds.min.is_finite()
            || !bounds.max.is_finite()
            || cells
                .iter()
                .any(|value| *value == 0 || *value > MAX_CELLS_PER_AXIS)
        {
            return Err(invalid(
                "quadric coefficients, bounds and cell counts must be finite and bounded",
            ));
        }
        Ok(Self {
            coefficients,
            bounds,
            cells,
        })
    }

    /// Coefficients in stable polynomial order.
    #[must_use]
    pub const fn coefficients(self) -> [f32; 10] {
        self.coefficients
    }

    /// Finite clipping and culling bounds.
    #[must_use]
    pub const fn bounds(self) -> Aabb {
        self.bounds
    }

    /// Deterministically lowers the zero-set to the shared mesh path.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidMesh`] when the zero-set does not cross the
    /// supplied bounds.
    pub fn to_mesh(
        self,
        owner: StructureHandle,
        color: Rgba8,
        material: Material,
    ) -> Result<Mesh, CoreError> {
        let mut vertices = Vec::new();
        let step = (self.bounds.max - self.bounds.min)
            / Vec3::new(
                f32::from(self.cells[0]),
                f32::from(self.cells[1]),
                f32::from(self.cells[2]),
            );
        for z in 0..self.cells[2] {
            for y in 0..self.cells[1] {
                for x in 0..self.cells[0] {
                    let base = self.bounds.min
                        + step * Vec3::new(f32::from(x), f32::from(y), f32::from(z));
                    let corners = cube_corners(base, step);
                    let values = corners.map(|point| self.evaluate(point));
                    for tetrahedron in TETRAHEDRA {
                        self.append_tetrahedron(corners, values, tetrahedron, color, &mut vertices);
                    }
                }
            }
        }
        Mesh::from_topology(owner, vertices, MeshTopology::Triangles, material)
    }

    fn evaluate(self, point: Vec3) -> f32 {
        let [xx, yy, zz, xy, xz, yz, x, y, z, constant] = self.coefficients;
        xx * point.x * point.x
            + yy * point.y * point.y
            + zz * point.z * point.z
            + xy * point.x * point.y
            + xz * point.x * point.z
            + yz * point.y * point.z
            + x * point.x
            + y * point.y
            + z * point.z
            + constant
    }

    fn normal(self, point: Vec3) -> Vec3 {
        let [xx, yy, zz, xy, xz, yz, x, y, z, _] = self.coefficients;
        Vec3::new(
            2.0 * xx * point.x + xy * point.y + xz * point.z + x,
            2.0 * yy * point.y + xy * point.x + yz * point.z + y,
            2.0 * zz * point.z + xz * point.x + yz * point.y + z,
        )
        .normalize_or_zero()
    }

    fn append_tetrahedron(
        self,
        corners: [Vec3; 8],
        values: [f32; 8],
        tetrahedron: [usize; 4],
        color: Rgba8,
        out: &mut Vec<MeshVertex>,
    ) {
        let mut crossings = Vec::with_capacity(4);
        for (left, right) in EDGES {
            let a = tetrahedron[left];
            let b = tetrahedron[right];
            if (values[a] < 0.0) == (values[b] < 0.0) {
                continue;
            }
            let t = (values[a] / (values[a] - values[b])).clamp(0.0, 1.0);
            let position = corners[a].lerp(corners[b], t);
            crossings.push(MeshVertex {
                position,
                normal: self.normal(position),
                color,
            });
        }
        match crossings.as_slice() {
            [a, b, c] => out.extend([*a, *b, *c]),
            [a, b, c, d] => out.extend([*a, *b, *c, *a, *c, *d]),
            _ => {}
        }
    }
}

fn cube_corners(base: Vec3, step: Vec3) -> [Vec3; 8] {
    [
        base,
        base + Vec3::new(step.x, 0.0, 0.0),
        base + Vec3::new(0.0, step.y, 0.0),
        base + Vec3::new(step.x, step.y, 0.0),
        base + Vec3::new(0.0, 0.0, step.z),
        base + Vec3::new(step.x, 0.0, step.z),
        base + Vec3::new(0.0, step.y, step.z),
        base + step,
    ]
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidPrimitive { reason }
}
