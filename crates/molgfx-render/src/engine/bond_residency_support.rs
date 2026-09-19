//! Endpoint lookup, dependency checks and placement validation for paged bonds.

use super::BondState;
use crate::engine::bond_draw_plan::ResidentAtomPage;
use crate::engine::{BondChunkPlacement, ChunkPlacementError, ChunkResidencyError};
use molgfx_core::{ChunkPayload, ChunkSpan, DatasetId, LogicalRow};

pub(super) fn resolve_endpoint(
    atoms: &[ResidentAtomPage],
    dataset_value: u64,
    row_value: u64,
) -> Result<u32, ChunkResidencyError> {
    let dataset = DatasetId::new(dataset_value);
    let row = LogicalRow::new(row_value);
    atoms
        .iter()
        .find_map(|page| page.resolve(dataset, row))
        .ok_or(ChunkResidencyError::EndpointNotResident { dataset, row })
}

pub(super) fn endpoints_resident(
    payload: &ChunkPayload,
    atoms: &[ResidentAtomPage],
) -> Result<bool, ChunkResidencyError> {
    let ChunkPayload::ProviderBond(bonds) = payload else {
        return Err(ChunkResidencyError::UnsupportedPayload);
    };
    for local in 0..bonds.descriptor().rows() {
        let value = record(bonds, local)?;
        if endpoint_missing(atoms, value.atom_a) || endpoint_missing(atoms, value.atom_b) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn depends_on(
    state: &BondState,
    dataset: DatasetId,
    span: ChunkSpan,
) -> Result<bool, ChunkResidencyError> {
    let payload = match state {
        BondState::Uploading { payload, .. }
        | BondState::Resident { payload, .. }
        | BondState::Blocked { payload, .. } => payload,
    };
    let ChunkPayload::ProviderBond(bonds) = payload else {
        return Err(ChunkResidencyError::UnsupportedPayload);
    };
    for local in 0..bonds.descriptor().rows() {
        let value = record(bonds, local)?;
        for endpoint in [value.atom_a, value.atom_b] {
            let row = LogicalRow::new(endpoint.row().get());
            if endpoint.dataset().get() == dataset.get()
                && span.local_row(molgfx_core::ChunkId::new(0), row).is_ok()
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn validate_bond_placements(
    placements: &[BondChunkPlacement],
    capacity: usize,
) -> Result<(), ChunkPlacementError> {
    if placements.len() > capacity {
        return Err(ChunkPlacementError::Capacity { capacity });
    }
    for (index, placement) in placements.iter().enumerate() {
        if !placement.model_to_world.is_finite() {
            return Err(ChunkPlacementError::NonFiniteTransform);
        }
        if !placement.radius.is_finite() || placement.radius <= 0.0 {
            return Err(ChunkPlacementError::InvalidBondRadius);
        }
        if placements[..index]
            .iter()
            .any(|value| value.id == placement.id)
        {
            return Err(ChunkPlacementError::DuplicateIdentity {
                id: placement.id.get(),
            });
        }
    }
    Ok(())
}

fn endpoint_missing(atoms: &[ResidentAtomPage], endpoint: molframe::AtomEndpoint) -> bool {
    resolve_endpoint(atoms, endpoint.dataset().get(), endpoint.row().get()).is_err()
}

fn record(
    bonds: &molframe::BondChunk,
    local: u32,
) -> Result<molframe::BondChunkRecord, ChunkResidencyError> {
    bonds
        .record(molframe::LocalRow::new(local))
        .map_err(|_| ChunkResidencyError::ProviderBondRecord { local_row: local })
}
impl<D: molgfx_gpu::Device> super::BondGpuResidency<D> {
    pub(crate) const fn binding_revision(&self) -> u64 {
        self.binding_revision
    }

    pub(crate) fn replace_binding_revision(&mut self, revision: u64) {
        self.binding_revision = revision;
    }

    pub(super) fn ensure_buffer(
        &mut self,
        device: &D,
    ) -> Result<(), crate::engine::ChunkResidencyError> {
        if !self.buffer_allocated {
            self.buffer = device.create_buffer(&molgfx_gpu::BufferDesc {
                label: "resident provider bonds",
                size: self.arena.capacity_bytes(),
                usage: molgfx_gpu::BufferUsage::STORAGE.union(molgfx_gpu::BufferUsage::COPY_DST),
            })?;
            self.buffer_allocated = true;
            self.binding_revision = self.binding_revision.wrapping_add(1).max(1);
        }
        Ok(())
    }
}
