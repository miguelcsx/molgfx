//! Structure placement reconciliation against shared physical asset buffers.

use super::GpuScene;
use crate::RenderError;
use crate::scene_gpu::asset::{GpuAsset, GpuAssetIdentity};
use crate::scene_gpu::structure::GpuStructure;
use molgfx_core::Scene;
use molgfx_gpu::Device;
use std::sync::Arc;

#[cfg(test)]
pub(crate) struct TestAssetLayout {
    pub(crate) buffers: [u32; 3],
    pub(crate) offsets: [u64; 3],
    pub(crate) lengths: [u64; 3],
    pub(crate) model: u32,
}

impl<D: Device> GpuScene<D> {
    pub(super) fn reconcile_structures(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let revision = scene.structure_revision();
        if self.structure_revision == Some(revision) {
            return Ok(false);
        }
        let mut old = std::mem::take(&mut self.structures);
        let mut uploaded = false;
        let arena_revision = self.asset_arena.revision();
        for (handle, placed) in scene.structures() {
            let identity = GpuAssetIdentity::from_placed(placed);
            self.validate_asset_identity(identity)?;
            let asset =
                if let Some(asset) = self.assets.iter().find(|asset| asset.identity == identity) {
                    Arc::clone(asset)
                } else {
                    let asset = Arc::new(GpuAsset::upload(
                        device,
                        queue,
                        &mut self.asset_arena,
                        placed,
                        identity,
                    )?);
                    self.assets.push(Arc::clone(&asset));
                    uploaded = true;
                    asset
                };
            if let Some(index) = old.iter().position(|gpu| gpu.handle == handle) {
                let mut structure = old.swap_remove(index);
                structure.replace_asset(asset);
                self.structures.push(structure);
            } else {
                self.structures.push(GpuStructure::new(handle, asset));
            }
        }
        drop(old);
        self.release_unused_assets()?;
        if self.asset_arena.revision() != arena_revision {
            for structure in &mut self.structures {
                structure.invalidate_asset_binding();
            }
        }
        self.structure_revision = Some(revision);
        Ok(uploaded)
    }

    fn release_unused_assets(&mut self) -> Result<(), RenderError> {
        let mut index = self.assets.len();
        while index > 0 {
            index -= 1;
            if Arc::strong_count(&self.assets[index]) == 1 {
                self.assets[index].release(&mut self.asset_arena)?;
                self.assets.swap_remove(index);
            }
        }
        Ok(())
    }

    pub(super) fn release_all_assets(&mut self) -> Result<(), RenderError> {
        for asset in &self.assets {
            asset.release(&mut self.asset_arena)?;
        }
        self.assets.clear();
        Ok(())
    }

    fn validate_asset_identity(&self, incoming: GpuAssetIdentity) -> Result<(), RenderError> {
        let conflict = self
            .assets
            .iter()
            .map(|asset| asset.identity)
            .find(|resident| resident.conflicts_with(incoming));
        match conflict {
            Some(resident) => Err(RenderError::AssetIdentityCollision {
                dataset: incoming.dataset,
                resident_fingerprint: resident.fingerprint,
                incoming_fingerprint: incoming.fingerprint,
            }),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    pub(crate) fn asset_counts(&self) -> (usize, usize, u64) {
        (
            self.assets.len(),
            self.structures.len(),
            self.assets.iter().map(|asset| asset.resident_bytes()).sum(),
        )
    }

    #[cfg(test)]
    pub(crate) fn asset_layouts<F>(&self, mut identify: F) -> Vec<TestAssetLayout>
    where
        F: FnMut(&D::Buffer) -> u32,
    {
        let arena_id = identify(self.asset_arena.buffer());
        self.structures
            .iter()
            .map(|structure| {
                let ranges = structure.test_ranges();
                let ids = [arena_id; 3];
                let offsets = ranges.map(super::super::asset_arena::AssetRange::offset);
                let lengths = ranges.map(super::super::asset_arena::AssetRange::allocated_bytes);
                let model = match structure.test_model() {
                    Some(buffer) => identify(buffer),
                    None => arena_id,
                };
                TestAssetLayout {
                    buffers: ids,
                    offsets,
                    lengths,
                    model,
                }
            })
            .collect()
    }
}
