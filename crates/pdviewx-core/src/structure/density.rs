//! Immutable grid-sampled scalar volumes with affine world placement.
//!
//! Construction validates the grid and computes its scalar range in
//! `O(voxels)`. Rendering can then upload the caller's shared allocation
//! directly without duplicating it in the scene.

use crate::{CoreError, ScalarFieldSemantics};
use pdviewx_math::{Aabb, Mat4, Quat, Vec3};
use std::sync::Arc;

#[cfg(test)]
#[path = "density_tests.rs"]
mod tests;

/// A row-major scalar grid, x-fastest, positioned by a full affine transform.
#[derive(Clone, Debug)]
pub struct ScalarVolume {
    dimensions: [u32; 3],
    empty_space_dimensions: [u32; 3],
    voxel_to_world: Mat4,
    values: Arc<[f32]>,
    empty_space_bounds: Arc<[f32]>,
    range: [f32; 2],
    semantics: ScalarFieldSemantics,
}

impl ScalarVolume {
    /// Number of source voxels grouped into one conservative skip cell.
    pub const EMPTY_SPACE_BRICK_SIZE: u32 = 8;

    /// Validates a scalar grid without copying its shared value allocation.
    ///
    /// # Errors
    ///
    /// Dimensions must be at least two and fit the portable 3-D texture
    /// ceiling, the value count must match their product, every value and
    /// matrix component must be finite, and the transform must be invertible.
    pub fn new(
        dimensions: [u32; 3],
        voxel_to_world: Mat4,
        values: Arc<[f32]>,
    ) -> Result<Self, CoreError> {
        if dimensions
            .iter()
            .any(|&dimension| !(2..=u32::from(u16::MAX)).contains(&dimension))
        {
            return Err(invalid("dimensions must be between 2 and 65535"));
        }
        let Some(voxels) = dimensions.iter().try_fold(1u64, |product, &dimension| {
            product.checked_mul(u64::from(dimension))
        }) else {
            return Err(invalid("dimension product overflows"));
        };
        if usize::try_from(voxels).ok() != Some(values.len()) {
            return Err(invalid("value count does not match dimensions"));
        }
        if !voxel_to_world
            .to_cols_array()
            .iter()
            .all(|component| component.is_finite())
            || !voxel_to_world.determinant().is_finite()
            || voxel_to_world.determinant().abs() <= 1e-8
        {
            return Err(invalid("voxel transform must be finite and invertible"));
        }
        let mut range = [f32::INFINITY, f32::NEG_INFINITY];
        for &value in values.iter() {
            if !value.is_finite() {
                return Err(invalid("density values must be finite"));
            }
            range[0] = range[0].min(value);
            range[1] = range[1].max(value);
        }
        let empty_space_dimensions = dimensions.map(|dimension| {
            dimension.saturating_add(Self::EMPTY_SPACE_BRICK_SIZE - 1)
                / Self::EMPTY_SPACE_BRICK_SIZE
        });
        let empty_space_bounds = Arc::from(build_empty_space_bounds(
            dimensions,
            empty_space_dimensions,
            &values,
        ));
        Ok(Self {
            dimensions,
            empty_space_dimensions,
            voxel_to_world,
            values,
            empty_space_bounds,
            range,
            semantics: ScalarFieldSemantics::default(),
        })
    }

    /// Builds an axis-aligned volume from an origin and positive voxel spacing.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::new`].
    pub fn from_spacing(
        dimensions: [u32; 3],
        origin: Vec3,
        spacing: Vec3,
        values: Arc<[f32]>,
    ) -> Result<Self, CoreError> {
        if !origin.is_finite() || !spacing.is_finite() || spacing.min_element() <= 0.0 {
            return Err(invalid("origin and positive spacing must be finite"));
        }
        Self::new(
            dimensions,
            Mat4::from_scale_rotation_translation(spacing, Quat::default(), origin),
            values,
        )
    }

    /// Grid dimensions in x, y and z order.
    #[must_use]
    pub const fn dimensions(&self) -> [u32; 3] {
        self.dimensions
    }

    /// Affine transform from voxel-index coordinates to world Ångström.
    #[must_use]
    pub const fn voxel_to_world(&self) -> Mat4 {
        self.voxel_to_world
    }

    /// Shared row-major scalar values; x varies fastest.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Dimensions of the conservative min/max macrocell grid.
    #[must_use]
    pub const fn empty_space_dimensions(&self) -> [u32; 3] {
        self.empty_space_dimensions
    }

    /// Interleaved finite min/max pairs for the macrocell grid, x-fastest.
    /// The renderer uses these bounds only to skip regions that cannot produce
    /// a visible transfer-function sample; it never replaces source sampling.
    #[must_use]
    pub fn empty_space_bounds(&self) -> &[f32] {
        &self.empty_space_bounds
    }

    /// Inclusive minimum and maximum values computed at construction.
    #[must_use]
    pub const fn range(&self) -> [f32; 2] {
        self.range
    }

    /// Attaches the caller's scientific meaning without changing grid data.
    #[must_use]
    pub fn with_semantics(mut self, semantics: ScalarFieldSemantics) -> Self {
        self.semantics = semantics;
        self
    }

    /// Scientific meaning and provenance of this grid.
    #[must_use]
    pub const fn semantics(&self) -> &ScalarFieldSemantics {
        &self.semantics
    }

    /// Tight world-space axis-aligned bound around the transformed grid.
    #[must_use]
    pub fn world_aabb(&self) -> Aabb {
        let maximum = Vec3::new(
            dimension_f32(self.dimensions[0] - 1),
            dimension_f32(self.dimensions[1] - 1),
            dimension_f32(self.dimensions[2] - 1),
        );
        Aabb::new(Vec3::ZERO, maximum).transform(&self.voxel_to_world)
    }
}

fn dimension_f32(value: u32) -> f32 {
    match u16::try_from(value) {
        Ok(value) => f32::from(value),
        Err(_) => f32::from(u16::MAX),
    }
}

fn build_empty_space_bounds(
    dimensions: [u32; 3],
    macro_dimensions: [u32; 3],
    values: &[f32],
) -> Vec<f32> {
    let macro_count = macro_dimensions
        .into_iter()
        .fold(1usize, |count, dimension| {
            count.saturating_mul(dimension as usize)
        });
    let mut bounds = Vec::with_capacity(macro_count.saturating_mul(2));
    for macro_z in 0..macro_dimensions[2] {
        for macro_y in 0..macro_dimensions[1] {
            for macro_x in 0..macro_dimensions[0] {
                let lower = [
                    macro_x * ScalarVolume::EMPTY_SPACE_BRICK_SIZE,
                    macro_y * ScalarVolume::EMPTY_SPACE_BRICK_SIZE,
                    macro_z * ScalarVolume::EMPTY_SPACE_BRICK_SIZE,
                ];
                // A sample inside this brick may interpolate with the first
                // texel of its positive neighbour. Including that one-texel
                // halo keeps min/max rejection conservative at brick edges.
                let upper = [
                    (lower[0] + ScalarVolume::EMPTY_SPACE_BRICK_SIZE + 1).min(dimensions[0]),
                    (lower[1] + ScalarVolume::EMPTY_SPACE_BRICK_SIZE + 1).min(dimensions[1]),
                    (lower[2] + ScalarVolume::EMPTY_SPACE_BRICK_SIZE + 1).min(dimensions[2]),
                ];
                let mut minimum = f32::INFINITY;
                let mut maximum = f32::NEG_INFINITY;
                for z in lower[2]..upper[2] {
                    for y in lower[1]..upper[1] {
                        let row = ((z * dimensions[1] + y) * dimensions[0] + lower[0]) as usize;
                        for x in 0..upper[0].saturating_sub(lower[0]) {
                            if let Some(&value) = values.get(row + x as usize) {
                                minimum = minimum.min(value);
                                maximum = maximum.max(value);
                            }
                        }
                    }
                }
                bounds.extend([minimum, maximum]);
            }
        }
    }
    bounds
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidVolume { reason }
}
