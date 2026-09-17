//! Persistent bounded GPU hash table for global brick addresses.

use super::types::{BrickAtlasError, BrickAtlasKind};
use molgfx_core::{BrickAddress, BrickDescriptor};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct BrickPageGpu {
    origin_lo_mip: [u32; 4],
    origin_hi_valid: [u32; 4],
    slot_shape: [u32; 4],
    generation_kind: [u32; 4],
}

impl BrickPageGpu {
    const fn empty() -> Self {
        Self {
            origin_lo_mip: [0; 4],
            origin_hi_valid: [0; 4],
            slot_shape: [0; 4],
            generation_kind: [0; 4],
        }
    }

    fn new(descriptor: BrickDescriptor, slot: u32, kind: BrickAtlasKind) -> Self {
        let metadata = descriptor.metadata;
        let origin = metadata.address.origin;
        let interior = metadata.shape.interior();
        let generation = metadata.generation.get();
        Self {
            origin_lo_mip: [
                low_word(origin[0]),
                low_word(origin[1]),
                low_word(origin[2]),
                u32::from(metadata.address.mip),
            ],
            origin_hi_valid: [
                high_word(origin[0]),
                high_word(origin[1]),
                high_word(origin[2]),
                1,
            ],
            slot_shape: [
                slot,
                u32::from(interior[0]),
                u32::from(interior[1]),
                u32::from(interior[2]),
            ],
            generation_kind: [
                low_word(generation),
                high_word(generation),
                u32::from(metadata.shape.halo()),
                kind.code(),
            ],
        }
    }
}

#[derive(Debug)]
pub(super) struct GpuPageTable {
    entries: Vec<BrickPageGpu>,
    mask: usize,
    mask_u32: u32,
}

impl GpuPageTable {
    pub(super) fn new(resident_capacity: usize) -> Result<Self, BrickAtlasError> {
        let Some(doubled) = resident_capacity.checked_mul(2) else {
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        let Some(capacity) = doubled.checked_next_power_of_two() else {
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        let Ok(mask_u32) = u32::try_from(capacity.saturating_sub(1)) else {
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        Ok(Self {
            entries: vec![BrickPageGpu::empty(); capacity],
            mask: capacity - 1,
            mask_u32,
        })
    }

    pub(super) fn rebuild(
        &mut self,
        published: &[Option<BrickDescriptor>],
        kind: BrickAtlasKind,
    ) -> Result<(), BrickAtlasError> {
        self.entries.fill(BrickPageGpu::empty());
        for (slot, descriptor) in published.iter().enumerate() {
            let Some(descriptor) = descriptor else {
                continue;
            };
            let Ok(local_slot) = u32::try_from(slot) else {
                return Err(BrickAtlasError::InvalidConfiguration);
            };
            self.insert(*descriptor, local_slot, kind)?;
        }
        Ok(())
    }

    pub(super) fn bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.entries)
    }

    pub(super) fn byte_len(&self) -> u64 {
        match u64::try_from(self.bytes().len()) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        }
    }

    pub(super) const fn mask(&self) -> u32 {
        self.mask_u32
    }

    fn insert(
        &mut self,
        descriptor: BrickDescriptor,
        slot: u32,
        kind: BrickAtlasKind,
    ) -> Result<(), BrickAtlasError> {
        let hash = hash_address(descriptor.metadata.address);
        let Ok(hash_index) = usize::try_from(hash) else {
            return Err(BrickAtlasError::InvalidConfiguration);
        };
        let mut index = hash_index & self.mask;
        for _ in 0..self.entries.len() {
            if self.entries[index].origin_hi_valid[3] == 0 {
                self.entries[index] = BrickPageGpu::new(descriptor, slot, kind);
                return Ok(());
            }
            index = (index + 1) & self.mask;
        }
        Err(BrickAtlasError::InvalidConfiguration)
    }
}

fn hash_address(address: BrickAddress) -> u32 {
    let mut hash = 2_166_136_261u32;
    for value in address.origin {
        hash = (hash ^ low_word(value)).wrapping_mul(16_777_619);
        hash = (hash ^ high_word(value)).wrapping_mul(16_777_619);
    }
    (hash ^ u32::from(address.mip)).wrapping_mul(16_777_619)
}

fn low_word(value: u64) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn high_word(value: u64) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])
}

pub(super) fn same_generation(left: BrickDescriptor, right: BrickDescriptor) -> bool {
    left.metadata.id == right.metadata.id && left.metadata.generation == right.metadata.generation
}
