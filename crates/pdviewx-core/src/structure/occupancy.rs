//! Compact declarations for GPU-resident temporal occupancy volumes.

use crate::CoreError;
use pdviewx_math::{Aabb, Mat4, Quat, Vec3};

#[cfg(test)]
#[path = "occupancy_tests.rs"]
mod tests;

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidVolume { reason }
}

/// GPU-resident temporal occupancy accumulation parameters.
///
/// The grid is defined in the selected structure's model space. The renderer
/// owns its evolving values; the scene retains only this compact declaration.
#[derive(Clone, Debug)]
pub struct OccupancyStream {
    dimensions: [u32; 3],
    voxel_to_model: Mat4,
    decay: f32,
    deposit: f32,
    maximum: f32,
}

impl OccupancyStream {
    /// Validates a fixed grid and finite accumulation controls.
    ///
    /// # Errors
    ///
    /// Dimensions must be in `[2, 65535]`; spacing, deposit and maximum must be
    /// positive and finite; decay must be finite in `[0, 1]`.
    pub fn new(
        dimensions: [u32; 3],
        origin: Vec3,
        spacing: Vec3,
        decay: f32,
        deposit: f32,
        maximum: f32,
    ) -> Result<Self, CoreError> {
        if dimensions
            .iter()
            .any(|&dimension| !(2..=u32::from(u16::MAX)).contains(&dimension))
        {
            return Err(invalid("occupancy dimensions must be between 2 and 65535"));
        }
        if !origin.is_finite() || !spacing.is_finite() || spacing.min_element() <= 0.0 {
            return Err(invalid(
                "occupancy origin and positive spacing must be finite",
            ));
        }
        if !decay.is_finite() || !(0.0..=1.0).contains(&decay) {
            return Err(invalid("occupancy decay must be finite in [0, 1]"));
        }
        if !deposit.is_finite()
            || deposit <= 0.0
            || !maximum.is_finite()
            || maximum <= 0.0
            || deposit > maximum
            || maximum > 1_000_000.0
        {
            return Err(invalid(
                "occupancy deposit must not exceed a positive maximum of 1,000,000",
            ));
        }
        dimensions
            .iter()
            .try_fold(1u64, |product, &dimension| {
                product.checked_mul(u64::from(dimension))
            })
            .ok_or_else(|| invalid("occupancy dimension product overflows"))?;
        Ok(Self {
            dimensions,
            voxel_to_model: Mat4::from_scale_rotation_translation(spacing, Quat::IDENTITY, origin),
            decay,
            deposit,
            maximum,
        })
    }

    /// Returns the grid dimensions in voxels.
    #[must_use]
    pub const fn dimensions(&self) -> [u32; 3] {
        self.dimensions
    }

    /// Returns the transform from voxel coordinates to model coordinates.
    #[must_use]
    pub const fn voxel_to_model(&self) -> Mat4 {
        self.voxel_to_model
    }

    /// Returns the multiplicative decay applied before each new sample.
    #[must_use]
    pub const fn decay(&self) -> f32 {
        self.decay
    }

    /// Returns the mass deposited per selected atom and sample.
    #[must_use]
    pub const fn deposit(&self) -> f32 {
        self.deposit
    }

    #[must_use]
    /// Returns the saturation ceiling applied to accumulated occupancy.
    pub const fn maximum(&self) -> f32 {
        self.maximum
    }

    #[must_use]
    /// Returns the grid extent in model coordinates.
    pub fn model_aabb(&self) -> Aabb {
        let maximum = Vec3::new(
            f32::from(u16::try_from(self.dimensions[0] - 1).map_or(u16::MAX, |value| value)),
            f32::from(u16::try_from(self.dimensions[1] - 1).map_or(u16::MAX, |value| value)),
            f32::from(u16::try_from(self.dimensions[2] - 1).map_or(u16::MAX, |value| value)),
        );
        Aabb::new(Vec3::ZERO, maximum).transform(&self.voxel_to_model)
    }
}
