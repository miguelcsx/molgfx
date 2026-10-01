//! Finite crystallographic cell geometry and guide boundaries.

use crate::{CoreError, SymmetryInstance};
use molgfx_math::{Mat3, Vec3};

#[cfg(test)]
#[path = "crystal_cell_tests.rs"]
mod tests;

const EPSILON: f32 = 1.0e-6;

/// A crystallographic unit cell in Cartesian ångström coordinates.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CrystalCell {
    lengths: [f32; 3],
    angles_degrees: [f32; 3],
    origin: Vec3,
    basis: Mat3,
}

impl CrystalCell {
    /// Creates a right-handed cell from lengths `[a, b, c]` and angles
    /// `[alpha, beta, gamma]` in degrees.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for non-finite,
    /// degenerate or geometrically impossible cell parameters.
    pub fn new(lengths: [f32; 3], angles_degrees: [f32; 3]) -> Result<Self, CoreError> {
        if lengths
            .iter()
            .any(|value| !value.is_finite() || *value <= EPSILON)
            || angles_degrees
                .iter()
                .any(|value| !value.is_finite() || *value <= EPSILON || *value >= 180.0 - EPSILON)
        {
            return Err(invalid(
                "cell lengths and angles must be finite and non-degenerate",
            ));
        }
        let [a, b, c] = lengths;
        let [alpha, beta, gamma] = angles_degrees.map(f32::to_radians);
        let sin_gamma = gamma.sin();
        if sin_gamma.abs() <= EPSILON {
            return Err(invalid("gamma produces a degenerate cell basis"));
        }
        let cos_alpha = alpha.cos();
        let cos_beta = beta.cos();
        let cos_gamma = gamma.cos();
        let a_axis = Vec3::new(a, 0.0, 0.0);
        let b_axis = Vec3::new(b * cos_gamma, b * sin_gamma, 0.0);
        let c_x = c * cos_beta;
        let c_y = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
        let c_z_sq = c.mul_add(c, -(c_x * c_x + c_y * c_y));
        if !c_z_sq.is_finite() || c_z_sq <= EPSILON {
            return Err(invalid("cell angles do not form a valid volume"));
        }
        Self {
            lengths,
            angles_degrees,
            origin: Vec3::ZERO,
            basis: Mat3::from_cols(a_axis, b_axis, Vec3::new(c_x, c_y, c_z_sq.sqrt())),
        }
        .with_origin(Vec3::ZERO)
    }

    /// Sets the Cartesian origin without changing the cell basis.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] if the translated boundary is
    /// non-finite or an edge collapses at Cartesian floating-point precision.
    pub fn with_origin(mut self, origin: Vec3) -> Result<Self, CoreError> {
        self.origin = origin;
        for (start, end) in self.edges() {
            if !start.is_finite() || !end.is_finite() || start.distance_squared(end) <= f32::EPSILON
            {
                return Err(invalid("cell boundary must be finite and non-degenerate"));
            }
        }
        Ok(self)
    }

    /// Cell lengths `[a, b, c]` in ångström.
    #[must_use]
    pub const fn lengths(self) -> [f32; 3] {
        self.lengths
    }

    /// Cell angles `[alpha, beta, gamma]` in degrees.
    #[must_use]
    pub const fn angles_degrees(self) -> [f32; 3] {
        self.angles_degrees
    }

    /// Cartesian origin of the cell.
    #[must_use]
    pub const fn origin(self) -> Vec3 {
        self.origin
    }

    /// Converts fractional coordinates into Cartesian coordinates.
    #[must_use]
    pub fn fractional_to_cartesian(self, fractional: [f32; 3]) -> Vec3 {
        self.origin + self.basis * Vec3::from_array(fractional)
    }

    /// The eight fractional corners, with x changing fastest.
    #[must_use]
    pub fn corners(self) -> [Vec3; 8] {
        [
            self.fractional_to_cartesian([0.0, 0.0, 0.0]),
            self.fractional_to_cartesian([1.0, 0.0, 0.0]),
            self.fractional_to_cartesian([0.0, 1.0, 0.0]),
            self.fractional_to_cartesian([1.0, 1.0, 0.0]),
            self.fractional_to_cartesian([0.0, 0.0, 1.0]),
            self.fractional_to_cartesian([1.0, 0.0, 1.0]),
            self.fractional_to_cartesian([0.0, 1.0, 1.0]),
            self.fractional_to_cartesian([1.0, 1.0, 1.0]),
        ]
    }

    /// Twelve unit-cell edges, suitable for lowering into analytic guides.
    #[must_use]
    pub fn edges(self) -> [(Vec3, Vec3); 12] {
        const EDGES: [(usize, usize); 12] = [
            (0, 1),
            (0, 2),
            (1, 3),
            (2, 3),
            (4, 5),
            (4, 6),
            (5, 7),
            (6, 7),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        let corners = self.corners();
        EDGES.map(|(start, end)| (corners[start], corners[end]))
    }

    /// Twelve edges after a caller-supplied symmetry/assembly transform.
    #[must_use]
    pub fn transformed_edges(self, instance: SymmetryInstance) -> [(Vec3, Vec3); 12] {
        self.edges().map(|(start, end)| {
            (
                instance.transform.transform_point3(start),
                instance.transform.transform_point3(end),
            )
        })
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidPrimitive { reason }
}
