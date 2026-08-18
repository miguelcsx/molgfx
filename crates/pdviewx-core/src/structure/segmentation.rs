//! Caller-owned categorical label volumes and reversible segment styles.

use crate::{CoreError, VolumeRegion, VolumeSlice};
use pdviewx_math::{Aabb, Mat4, Quat, Rgba8, Vec3};
use std::sync::Arc;

#[cfg(test)]
#[path = "segmentation_tests.rs"]
mod tests;

/// A row-major, x-fastest integer label grid with affine world placement.
#[derive(Clone, Debug)]
pub struct SegmentedVolume {
    dimensions: [u32; 3],
    voxel_to_world: Mat4,
    labels: Arc<[u32]>,
}

impl SegmentedVolume {
    /// Validates a categorical grid without copying its shared label storage.
    ///
    /// Dimensions must be at least two and fit the portable 3-D texture
    /// ceiling. Labels are opaque caller data, so every `u32` value is valid.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSegmentation`] when dimensions, label
    /// count, or the voxel transform is invalid.
    pub fn new(
        dimensions: [u32; 3],
        voxel_to_world: Mat4,
        labels: Arc<[u32]>,
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
        if usize::try_from(voxels).ok() != Some(labels.len()) {
            return Err(invalid("label count does not match dimensions"));
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
        Ok(Self {
            dimensions,
            voxel_to_world,
            labels,
        })
    }

    /// Builds an axis-aligned categorical grid from an origin and positive
    /// voxel spacing.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSegmentation`] when the origin or
    /// spacing is non-finite, spacing is not positive, or the labels do
    /// not match the requested dimensions.
    pub fn from_spacing(
        dimensions: [u32; 3],
        origin: Vec3,
        spacing: Vec3,
        labels: Arc<[u32]>,
    ) -> Result<Self, CoreError> {
        if !origin.is_finite() || !spacing.is_finite() || spacing.min_element() <= 0.0 {
            return Err(invalid("origin and positive spacing must be finite"));
        }
        Self::new(
            dimensions,
            Mat4::from_scale_rotation_translation(spacing, Quat::default(), origin),
            labels,
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

    /// Shared row-major labels; x varies fastest.
    #[must_use]
    pub fn labels(&self) -> &[u32] {
        &self.labels
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

/// One independent visual style for one integer label.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SegmentStyle {
    /// Exact caller-supplied voxel label.
    pub label: u32,
    /// Reversible display color.
    pub color: Rgba8,
    /// Optical opacity in [0, 1]; zero hides the label.
    pub opacity: f32,
}

impl SegmentStyle {
    /// Creates one label style.
    #[must_use]
    pub const fn new(label: u32, color: Rgba8, opacity: f32) -> Self {
        Self {
            label,
            color,
            opacity,
        }
    }
}

/// Immutable, sorted, shareable styles for a categorical volume.
#[derive(Clone, PartialEq, Debug)]
pub struct SegmentStyleTable {
    styles: Arc<[SegmentStyle]>,
}

impl SegmentStyleTable {
    /// Validates and sorts styles by label.
    ///
    /// # Errors
    ///
    /// Labels must be unique and opacities must be finite in [0, 1].
    pub fn new(styles: &[SegmentStyle]) -> Result<Self, CoreError> {
        if styles
            .iter()
            .any(|style| !style.opacity.is_finite() || !(0.0..=1.0).contains(&style.opacity))
        {
            return Err(invalid("segment opacity must be finite in [0, 1]"));
        }
        let mut sorted = styles.to_vec();
        sorted.sort_unstable_by_key(|style| style.label);
        if sorted.windows(2).any(|pair| pair[0].label == pair[1].label) {
            return Err(invalid("segment labels must be unique"));
        }
        Ok(Self {
            styles: Arc::from(sorted.into_boxed_slice()),
        })
    }

    /// Styles in deterministic ascending-label order.
    #[must_use]
    pub fn styles(&self) -> &[SegmentStyle] {
        &self.styles
    }

    /// Binary-searches one style by exact label.
    #[must_use]
    pub fn style_for(&self, label: u32) -> Option<SegmentStyle> {
        self.styles
            .binary_search_by_key(&label, |style| style.label)
            .ok()
            .and_then(|index| self.styles.get(index).copied())
    }
}

impl Default for SegmentStyleTable {
    fn default() -> Self {
        Self {
            styles: Arc::from([]),
        }
    }
}

/// Sampling and clipping controls for one categorical representation.
#[derive(Clone, PartialEq, Debug)]
pub struct SegmentationStyle {
    /// Independent label-to-color-and-opacity mapping.
    pub styles: SegmentStyleTable,
    /// Global multiplier applied after the per-label opacity.
    pub opacity_scale: f32,
    /// Ray step relative to the smallest voxel axis.
    pub step_scale: f32,
    /// Optional world-space plane sampled as one categorical slice.
    pub slice: Option<VolumeSlice>,
    /// Optional half-open voxel region rendered from the resident grid.
    pub region: Option<VolumeRegion>,
}

impl Default for SegmentationStyle {
    fn default() -> Self {
        Self {
            styles: SegmentStyleTable::default(),
            opacity_scale: 1.0,
            step_scale: 0.65,
            slice: None,
            region: None,
        }
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidSegmentation { reason }
}

fn dimension_f32(value: u32) -> f32 {
    match u16::try_from(value) {
        Ok(value) => f32::from(value),
        Err(_) => f32::from(u16::MAX),
    }
}
