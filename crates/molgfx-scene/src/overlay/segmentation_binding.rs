//! Shared categorical voxel storage stays outside the portable scene document.

use super::{DataSource, SegmentationSpec};
use crate::Error;
use molgfx_math::Mat4;
use std::sync::Arc;

/// Immutable labels bound by content hash, without copying caller-owned storage.
#[derive(Clone, Debug)]
pub struct SegmentationBinding {
    source: DataSource,
    grid: molgfx_core::SegmentedVolume,
}

impl SegmentationBinding {
    /// Validates and retains an immutable affine label grid.
    ///
    /// # Errors
    /// Returns an error for invalid provenance, grid dimensions, affine geometry
    /// or a label count that differs from the dimension product.
    pub fn new(
        source: DataSource,
        dimensions: [u32; 3],
        voxel_to_world: [f32; 16],
        labels: Arc<[u32]>,
    ) -> Result<Self, Error> {
        source.validate()?;
        super::volume_spec::validate_affine(voxel_to_world)?;
        let grid = molgfx_core::SegmentedVolume::new(
            dimensions,
            Mat4::from_cols_array(&voxel_to_world),
            labels,
        )?;
        Ok(Self { source, grid })
    }

    /// Content identity used by portable categorical descriptors.
    #[must_use]
    pub fn content_hash(&self) -> &str {
        &self.source.content_hash
    }

    /// Shared voxel labels in x-fastest order.
    #[must_use]
    pub fn labels(&self) -> &[u32] {
        self.grid.labels()
    }

    pub(crate) fn matches(&self, spec: &SegmentationSpec) -> Result<(), Error> {
        if self.grid.dimensions() != spec.dimensions
            || self.grid.voxel_to_world().to_cols_array().map(f32::to_bits)
                != spec.voxel_to_world.map(f32::to_bits)
        {
            return Err(Error::InvalidSpec(format!(
                "segmentation source '{}' does not match its descriptor",
                self.content_hash()
            )));
        }
        Ok(())
    }

    pub(crate) fn native(&self) -> molgfx_core::SegmentedVolume {
        self.grid.clone()
    }
}
