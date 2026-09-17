//! Validated caller-authored planar regions.

use crate::{CoreError, StructureHandle};
use molgfx_math::Vec3;

#[cfg(test)]
#[path = "planar_tests.rs"]
mod tests;

const EPSILON: f32 = 1.0e-6;

/// A finite rectangular region in a structure's model space.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PlanarRegion {
    /// Structure whose placement owns the region.
    pub owner: StructureHandle,
    /// Center in model space.
    pub center: Vec3,
    /// Unit surface normal.
    pub normal: Vec3,
    /// Unit in-plane tangent.
    pub tangent: Vec3,
    /// Unit in-plane bitangent.
    pub bitangent: Vec3,
    /// Full dimensions along tangent and bitangent.
    pub size: [f32; 2],
}

impl PlanarRegion {
    /// Constructs a region from a normal and an approximate in-plane tangent.
    ///
    /// The tangent is orthogonalized deterministically, so callers can supply
    /// a crystal axis or camera-independent annotation direction without
    /// having to normalize it first.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for degenerate pose
    /// or dimensions.
    pub fn new(
        owner: StructureHandle,
        center: Vec3,
        normal: Vec3,
        tangent: Vec3,
        size: [f32; 2],
    ) -> Result<Self, CoreError> {
        if !center.is_finite()
            || !normal.is_finite()
            || !tangent.is_finite()
            || !size
                .iter()
                .all(|value| value.is_finite() && *value > EPSILON)
            || normal.length_squared() <= EPSILON
        {
            return Err(invalid(
                "planar region pose and size must be finite and non-degenerate",
            ));
        }
        let normal = normal.normalize();
        let tangent = tangent - normal * normal.dot(tangent);
        if tangent.length_squared() <= EPSILON {
            return Err(invalid(
                "planar region tangent must not be parallel to its normal",
            ));
        }
        let tangent = tangent.normalize();
        Ok(Self {
            owner,
            center,
            normal,
            tangent,
            bitangent: normal.cross(tangent).normalize(),
            size,
        })
    }

    /// Four corners in winding order.
    #[must_use]
    pub fn corners(self) -> [Vec3; 4] {
        let half_tangent = self.tangent * (self.size[0] * 0.5);
        let half_bitangent = self.bitangent * (self.size[1] * 0.5);
        [
            self.center - half_tangent - half_bitangent,
            self.center + half_tangent - half_bitangent,
            self.center + half_tangent + half_bitangent,
            self.center - half_tangent + half_bitangent,
        ]
    }

    /// Four boundary segments suitable for the existing guide path.
    #[must_use]
    pub fn edges(self) -> [(Vec3, Vec3); 4] {
        let corners = self.corners();
        [
            (corners[0], corners[1]),
            (corners[1], corners[2]),
            (corners[2], corners[3]),
            (corners[3], corners[0]),
        ]
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidPrimitive { reason }
}
