//! Immutable GPU resources shared by every placement of one structure asset.

use super::asset_arena::{AssetArena, AssetRange};
use crate::RenderError;
use pdviewx_core::{DatasetId, PlacedStructure};
use pdviewx_gpu::Device;

#[cfg(test)]
#[path = "asset_tests.rs"]
mod tests;

/// Collision-resistant identity for immutable renderer input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GpuAssetIdentity {
    pub(super) dataset: DatasetId,
    pub(super) coordinate_generation: u64,
    pub(super) fingerprint: u64,
}

impl GpuAssetIdentity {
    pub(super) fn from_placed(placed: &PlacedStructure) -> Self {
        let mut fingerprint = Fingerprint::new();
        fingerprint.bytes(placed.atoms.coords().as_bytes());
        fingerprint.column(placed.atoms.element().values());
        fingerprint.column(placed.atoms.residue().values());
        fingerprint.column(placed.atoms.radius().values());
        fingerprint.column(placed.atoms.color().values());
        fingerprint.column(placed.atoms.flags().values());
        fingerprint.column(placed.atoms.semantic().values());
        Self {
            dataset: placed.dataset_id(),
            coordinate_generation: placed.atoms.coords().generation(),
            fingerprint: fingerprint.finish(),
        }
    }

    pub(super) fn conflicts_with(self, other: Self) -> bool {
        self.dataset == other.dataset
            && self.coordinate_generation == other.coordinate_generation
            && self.fingerprint != other.fingerprint
    }
}

/// Physical buffers for one immutable structure asset.
#[derive(Debug)]
pub(super) struct GpuAsset<D: Device> {
    pub(super) identity: GpuAssetIdentity,
    coordinates: AssetRange,
    bvh_nodes: AssetRange,
    bvh_indices: AssetRange,
    resident_bytes: u64,
    _device: std::marker::PhantomData<D>,
}

impl<D: Device> GpuAsset<D> {
    pub(super) fn upload(
        device: &D,
        queue: &D::Queue,
        arena: &mut AssetArena<D>,
        placed: &PlacedStructure,
        identity: GpuAssetIdentity,
    ) -> Result<Self, RenderError> {
        let coordinate_bytes = placed.atoms.coords().as_bytes();
        let bvh = placed.spatial_bvh()?;
        let mut packed_indices = Vec::with_capacity(
            bvh.primitive_indices
                .len()
                .checked_add(bvh.escape.len())
                .ok_or(pdviewx_gpu::GpuError::LimitExceeded {
                    resource: "asset BVH index count",
                    limit: u64::from(u32::MAX),
                })?,
        );
        packed_indices.extend_from_slice(&bvh.primitive_indices);
        packed_indices.extend_from_slice(&bvh.escape);
        let coordinates = arena.upload(device, queue, coordinate_bytes)?;
        let bvh_nodes = match arena.upload(device, queue, bytemuck::cast_slice(&bvh.nodes)) {
            Ok(range) => range,
            Err(error) => {
                arena.release(coordinates)?;
                return Err(error.into());
            }
        };
        let bvh_indices = match arena.upload(device, queue, bytemuck::cast_slice(&packed_indices)) {
            Ok(range) => range,
            Err(error) => {
                arena.release(bvh_nodes)?;
                arena.release(coordinates)?;
                return Err(error.into());
            }
        };
        let resident_bytes = coordinates
            .allocated_bytes()
            .checked_add(bvh_nodes.allocated_bytes())
            .and_then(|bytes| bytes.checked_add(bvh_indices.allocated_bytes()))
            .ok_or(pdviewx_gpu::GpuError::LimitExceeded {
                resource: "asset resident byte count",
                limit: u64::MAX,
            })?;
        Ok(Self {
            identity,
            coordinates,
            bvh_nodes,
            bvh_indices,
            resident_bytes,
            _device: std::marker::PhantomData,
        })
    }

    pub(super) const fn coordinates(&self) -> AssetRange {
        self.coordinates
    }

    pub(super) const fn bvh_nodes(&self) -> AssetRange {
        self.bvh_nodes
    }

    pub(super) const fn bvh_indices(&self) -> AssetRange {
        self.bvh_indices
    }

    pub(super) const fn resident_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub(super) fn release(&self, arena: &mut AssetArena<D>) -> Result<(), RenderError> {
        arena.release(self.coordinates)?;
        arena.release(self.bvh_nodes)?;
        arena.release(self.bvh_indices)?;
        Ok(())
    }
}

struct Fingerprint(u64);

impl Fingerprint {
    const fn new() -> Self {
        Self(14_695_981_039_346_656_037)
    }

    fn column<T: bytemuck::Pod>(&mut self, values: &[T]) {
        self.bytes(bytemuck::cast_slice(values));
    }

    fn bytes(&mut self, values: &[u8]) {
        for value in values {
            self.0 ^= u64::from(*value);
            self.0 = self.0.wrapping_mul(1_099_511_628_211);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
