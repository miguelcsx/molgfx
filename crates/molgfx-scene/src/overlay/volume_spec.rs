//! Density-volume metadata and its builder.

use super::DataSource;
use crate::Color;
use serde::{Deserialize, Serialize};

/// Density-volume metadata. Grid values are supplied through a runtime binding.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VolumeSpec {
    /// Portable source descriptor for the grid values.
    pub source: DataSource,
    /// Voxel dimensions along x, y, and z.
    pub dimensions: [u32; 3],
    /// Voxel spacing in ångström.
    pub spacing: [f32; 3],
    /// World-space origin in ångström.
    pub origin: [f32; 3],
    /// Density isovalue used by the default presentation.
    pub isovalue: f32,
    /// Default presentation color.
    pub color: Color,
}

/// Immutable density-volume builder.
#[derive(Clone, PartialEq, Debug)]
pub struct Volume(pub(super) VolumeSpec);

impl Volume {
    /// Sets positive voxel spacing in ångström.
    #[must_use]
    pub fn spacing(mut self, spacing: [f32; 3]) -> Self {
        self.0.spacing = spacing;
        self
    }

    /// Sets the world-space grid origin.
    #[must_use]
    pub fn origin(mut self, origin: [f32; 3]) -> Self {
        self.0.origin = origin;
        self
    }

    /// Sets the default finite isovalue.
    #[must_use]
    pub fn isovalue(mut self, isovalue: f32) -> Self {
        self.0.isovalue = isovalue;
        self
    }

    /// Sets the default volume color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.0.color = color;
        self
    }
}
