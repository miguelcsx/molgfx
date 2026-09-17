//! Declarative volume-presentation recipes.

use super::{VolumeRegion, VolumeRendering, VolumeSlice, VolumeStyle, VolumeTransferFunction};

impl VolumeStyle {
    /// Applies an explicit scalar-to-color-and-opacity transfer function.
    #[must_use]
    pub fn transfer(mut self, transfer: VolumeTransferFunction) -> Self {
        self.transfer = transfer;
        self
    }

    /// Lit scalar isosurface sampling.
    #[must_use]
    pub fn isosurface() -> Self {
        Self {
            rendering: VolumeRendering::Isosurface,
            ..Self::default()
        }
    }

    /// Single-scattered participating medium.
    #[must_use]
    pub fn medium() -> Self {
        Self {
            rendering: VolumeRendering::Medium,
            ..Self::default()
        }
    }

    /// Screen-space liquid boundary over the scalar field.
    #[must_use]
    pub fn liquid_surface() -> Self {
        Self {
            rendering: VolumeRendering::LiquidSurface,
            ..Self::default()
        }
    }

    /// One arbitrary transfer-mapped plane through the field.
    #[must_use]
    pub fn slice(slice: VolumeSlice) -> Self {
        Self {
            rendering: VolumeRendering::Slice,
            slice: Some(slice),
            ..Self::default()
        }
    }

    /// Restricts sampling to one half-open voxel region.
    #[must_use]
    pub const fn region(mut self, region: VolumeRegion) -> Self {
        self.region = Some(region);
        self
    }

    /// Sets optical density and relative ray-step length.
    #[must_use]
    pub const fn sampling(mut self, opacity_scale: f32, step_scale: f32) -> Self {
        self.opacity_scale = opacity_scale;
        self.step_scale = step_scale;
        self
    }
}
