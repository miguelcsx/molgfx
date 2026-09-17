//! Caller-driven sparse brick delivery without renderer-owned I/O.

use super::Engine;
use crate::{BrickAtlasConfig, BrickAtlasError, BrickAtlasMetrics, BrickAtlasUpload};
use molgfx_core::{BrickCatalog, BrickId};
use molgfx_gpu::{Device, FenceValue};

impl<D: Device> Engine<D> {
    /// Allocates a bounded atlas for one logical sparse field.
    ///
    /// # Errors
    ///
    /// Returns typed catalog, capacity or GPU limit failures.
    pub fn install_brick_atlas(
        &mut self,
        catalog: &BrickCatalog,
        config: BrickAtlasConfig,
    ) -> Result<usize, BrickAtlasError> {
        self.scene_gpu
            .install_brick_atlas(&self.device, &self.queue, catalog, config)
    }

    /// Delivers one borrowed provider payload to persistent staging.
    ///
    /// # Errors
    ///
    /// Returns typed atlas index, generation or upload backpressure.
    pub fn stage_brick(
        &mut self,
        atlas: usize,
        upload: BrickAtlasUpload<'_>,
    ) -> Result<FenceValue, BrickAtlasError> {
        let capacity = self.scene_gpu.brick_atlas_count();
        let Some(target) = self.scene_gpu.brick_atlas_mut(atlas) else {
            return Err(BrickAtlasError::LifecycleCapacity { capacity });
        };
        target.stage(&self.device, &self.queue, upload)
    }

    /// Invalidates a page and schedules fence-safe slot release.
    ///
    /// # Errors
    ///
    /// Returns typed atlas index, page or lifecycle failures.
    pub fn evict_brick(
        &mut self,
        atlas: usize,
        brick: BrickId,
    ) -> Result<FenceValue, BrickAtlasError> {
        let capacity = self.scene_gpu.brick_atlas_count();
        let Some(target) = self.scene_gpu.brick_atlas_mut(atlas) else {
            return Err(BrickAtlasError::LifecycleCapacity { capacity });
        };
        target.request_eviction(&self.device, &self.queue, brick)
    }

    /// Current bounded memory and lifecycle counters for one atlas.
    #[must_use]
    pub fn brick_atlas_metrics(&mut self, atlas: usize) -> Option<BrickAtlasMetrics> {
        self.scene_gpu
            .brick_atlas_mut(atlas)
            .map(|target| target.metrics())
    }
}
