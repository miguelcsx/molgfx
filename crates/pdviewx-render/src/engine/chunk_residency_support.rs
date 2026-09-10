//! Validation and fixed-buffer construction for paged structure residency.

use super::{
    ChunkPlacementError, ChunkResidencyError, InstanceChunkPlacement, PointChunkPlacement,
    RelationChunkPlacement, StructureChunkPlacement,
};
use hashbrown::HashSet;
use pdviewx_gpu::{BufferDesc, BufferUsage, Device};

pub(super) fn create_coordinate_buffer<D: Device>(
    device: &D,
    size: u64,
    label: &'static str,
) -> Result<D::Buffer, ChunkResidencyError> {
    Ok(device.create_buffer(&BufferDesc {
        label,
        size,
        usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
    })?)
}

pub(super) fn validate_instance_placements(
    placements: &[InstanceChunkPlacement],
    capacity: usize,
) -> Result<(), ChunkPlacementError> {
    if placements.len() > capacity {
        return Err(ChunkPlacementError::Capacity { capacity });
    }
    let mut identities = HashSet::with_capacity(placements.len());
    for placement in placements {
        if !identities.insert(placement.id()) {
            return Err(ChunkPlacementError::DuplicateIdentity {
                id: placement.id().get(),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_relation_placements(
    placements: &[RelationChunkPlacement],
    capacity: usize,
) -> Result<(), ChunkPlacementError> {
    if placements.len() > capacity {
        return Err(ChunkPlacementError::Capacity { capacity });
    }
    let mut identities = HashSet::with_capacity(placements.len());
    for placement in placements {
        if !identities.insert(placement.id()) {
            return Err(ChunkPlacementError::DuplicateIdentity {
                id: placement.id().get(),
            });
        }
    }
    Ok(())
}

pub(super) fn create_cluster_buffer<D: Device>(
    device: &D,
    size: u64,
) -> Result<D::Buffer, ChunkResidencyError> {
    Ok(device.create_buffer(&BufferDesc {
        label: "resident structure chunk clusters",
        size,
        usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
    })?)
}

pub(super) fn cluster_capacity(
    bytes: u64,
    machine_capacity: usize,
) -> Result<u32, ChunkResidencyError> {
    let rows = bytes / 16;
    let clusters = rows.div_ceil(64).saturating_add(
        u64::try_from(machine_capacity).map_err(|_| ChunkResidencyError::SizeOverflow)?,
    );
    u32::try_from(clusters).map_err(|_| ChunkResidencyError::LocalAddressOverflow)
}

pub(super) fn local_offset<T>(bytes: u64) -> Result<u32, ChunkResidencyError> {
    let stride = std::mem::size_of::<T>() as u64;
    if !bytes.is_multiple_of(stride) {
        return Err(ChunkResidencyError::LocalAddressOverflow);
    }
    u32::try_from(bytes / stride).map_err(|_| ChunkResidencyError::LocalAddressOverflow)
}

pub(super) fn validate_placements(
    placements: &[StructureChunkPlacement],
    capacity: usize,
) -> Result<(), ChunkPlacementError> {
    if placements.len() > capacity {
        return Err(ChunkPlacementError::Capacity { capacity });
    }
    let mut identities = HashSet::with_capacity(placements.len());
    for placement in placements {
        if !placement.model_to_world.is_finite() {
            return Err(ChunkPlacementError::NonFiniteTransform);
        }
        if !identities.insert(placement.id) {
            return Err(ChunkPlacementError::DuplicateIdentity {
                id: placement.id.get(),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_point_placements(
    placements: &[PointChunkPlacement],
    capacity: usize,
) -> Result<(), ChunkPlacementError> {
    if placements.len() > capacity {
        return Err(ChunkPlacementError::Capacity { capacity });
    }
    let mut identities = HashSet::with_capacity(placements.len());
    for placement in placements {
        if !placement.model_to_world().is_finite() {
            return Err(ChunkPlacementError::NonFiniteTransform);
        }
        if !identities.insert(placement.id()) {
            return Err(ChunkPlacementError::DuplicateIdentity {
                id: placement.id().get(),
            });
        }
    }
    Ok(())
}
