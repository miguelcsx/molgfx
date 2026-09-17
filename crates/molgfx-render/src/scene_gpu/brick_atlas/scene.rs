//! Sparse-atlas ownership and lifecycle entry points on the resident scene.

use super::types::{BrickAtlasConfig, BrickAtlasError};
use super::upload::GpuBrickAtlas;
use crate::scene_gpu::GpuScene;
use molgfx_core::BrickCatalog;
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
    /// Creates one catalog-validated atlas outside the frame loop.
    ///
    /// # Errors
    ///
    /// Returns typed atlas validation, capacity or device failures.
    pub fn install_brick_atlas(
        &mut self,
        device: &D,
        queue: &D::Queue,
        catalog: &BrickCatalog,
        config: BrickAtlasConfig,
    ) -> Result<usize, BrickAtlasError> {
        let atlas = GpuBrickAtlas::new(device, queue, catalog, config)?;
        self.brick_atlases.push(atlas);
        Ok(self.brick_atlases.len().saturating_sub(1))
    }

    /// Mutable atlas access used by provider completion and fence polling.
    #[must_use]
    pub fn brick_atlas_mut(&mut self, index: usize) -> Option<&mut GpuBrickAtlas<D>> {
        self.brick_atlases.get_mut(index)
    }

    /// Number of independently budgeted sparse fields owned by the scene.
    #[must_use]
    pub const fn brick_atlas_count(&self) -> usize {
        self.brick_atlases.len()
    }
}
