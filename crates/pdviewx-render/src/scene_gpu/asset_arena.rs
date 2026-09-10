//! One physical storage buffer with stable, aligned immutable suballocations.
//!
//! Asset upload is `O(payload bytes)`. Growth is `O(resident physical bytes)`
//! on the GPU through a buffer copy. Stable frames perform neither operation.

use crate::ImmutableArenaMetrics;
use pdviewx_gpu::{
    ArenaAllocation, ArenaError, BindGroupEntry, BufferDesc, BufferUsage, CommandEncoder, Device,
    PagedArena, Queue,
};
use thiserror::Error;

const STORAGE_ALIGNMENT: u64 = 256;
const INITIAL_ARENA_BYTES: u64 = 1 << 20;

/// A generation-checked range in the immutable GPU asset arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AssetRange {
    allocation: ArenaAllocation,
    binding_bytes: u64,
}

impl AssetRange {
    pub(super) const fn offset(self) -> u64 {
        self.allocation.byte_offset()
    }

    pub(super) const fn binding_bytes(self) -> u64 {
        self.binding_bytes
    }

    pub(super) fn allocated_bytes(self) -> u64 {
        u64::from(self.allocation.page_count()) * STORAGE_ALIGNMENT
    }
}

/// Physical counters for immutable asset storage only.
/// Capacity and allocator failures remain distinguishable from device loss.
#[derive(Debug, Error)]
pub enum AssetArenaError {
    /// Allocator metadata rejected the operation.
    #[error(transparent)]
    Allocator(#[from] ArenaError),
    /// The device cannot host a larger physical immutable arena.
    #[error("immutable GPU arena cannot fit {requested_bytes} bytes within {capacity_bytes} bytes")]
    Backpressure {
        /// Payload that triggered growth.
        requested_bytes: u64,
        /// Maximum physical buffer capacity exposed by the device.
        capacity_bytes: u64,
    },
    /// Physical byte arithmetic overflowed.
    #[error("immutable GPU arena size overflow")]
    SizeOverflow,
    /// Buffer creation or submission failed.
    #[error(transparent)]
    Gpu(#[from] pdviewx_gpu::GpuError),
}

#[derive(Debug)]
pub(super) struct AssetArena<D: Device> {
    buffer: D::Buffer,
    allocator: PagedArena,
    max_bytes: u64,
    revision: u64,
    metrics: ImmutableArenaMetrics,
}

impl<D: Device> AssetArena<D> {
    pub(super) fn new(device: &D) -> Result<Self, AssetArenaError> {
        Self::with_initial_bytes(device, INITIAL_ARENA_BYTES)
    }

    fn with_initial_bytes(device: &D, requested: u64) -> Result<Self, AssetArenaError> {
        let max_bytes = aligned_capacity(device.capabilities().max_storage_buffer_bytes)?;
        let initial = requested.max(STORAGE_ALIGNMENT).min(max_bytes);
        let pages = page_count(initial)?;
        let physical_bytes = u64::from(pages) * STORAGE_ALIGNMENT;
        let buffer = create_buffer(device, physical_bytes)?;
        Ok(Self {
            buffer,
            allocator: PagedArena::new(STORAGE_ALIGNMENT, pages)?,
            max_bytes,
            revision: 1,
            metrics: ImmutableArenaMetrics {
                physical_buffers: 1,
                physical_bytes,
                ..ImmutableArenaMetrics::default()
            },
        })
    }

    pub(super) fn upload(
        &mut self,
        device: &D,
        queue: &D::Queue,
        bytes: &[u8],
    ) -> Result<AssetRange, AssetArenaError> {
        let binding_bytes = binding_size(bytes.len())?;
        let allocation = match self.allocator.allocate(binding_bytes) {
            Ok(allocation) => allocation,
            Err(ArenaError::OutOfMemory { .. }) => {
                self.grow(device, queue, binding_bytes)?;
                self.allocator.allocate(binding_bytes)?
            }
            Err(error) => return Err(error.into()),
        };
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, allocation.byte_offset(), bytes);
            self.metrics.writes = self.metrics.writes.saturating_add(1);
            self.metrics.uploaded_bytes = self
                .metrics
                .uploaded_bytes
                .saturating_add(bytes.len() as u64);
        }
        self.refresh_resident_metrics();
        Ok(AssetRange {
            allocation,
            binding_bytes,
        })
    }

    pub(super) fn release(&mut self, range: AssetRange) -> Result<(), AssetArenaError> {
        self.allocator.release(range.allocation)?;
        self.refresh_resident_metrics();
        Ok(())
    }

    pub(super) fn entry(&self, binding: u32, range: AssetRange) -> BindGroupEntry<'_, D> {
        BindGroupEntry::BufferRange {
            binding,
            buffer: &self.buffer,
            offset: range.offset(),
            size: range.binding_bytes(),
        }
    }

    #[cfg(test)]
    pub(super) const fn buffer(&self) -> &D::Buffer {
        &self.buffer
    }

    pub(super) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) const fn metrics(&self) -> ImmutableArenaMetrics {
        self.metrics
    }

    fn grow(
        &mut self,
        device: &D,
        queue: &D::Queue,
        requested_bytes: u64,
    ) -> Result<(), AssetArenaError> {
        let old_bytes = self.allocator.capacity_bytes();
        let requested_pages = page_count(requested_bytes)?;
        let current_pages = self.allocator.page_count();
        let desired_extra = current_pages.max(requested_pages);
        let max_pages = page_count(self.max_bytes)?;
        let available = max_pages.saturating_sub(current_pages);
        if available == 0 || available < requested_pages {
            self.metrics.allocation_stalls = self.metrics.allocation_stalls.saturating_add(1);
            return Err(AssetArenaError::Backpressure {
                requested_bytes,
                capacity_bytes: self.max_bytes,
            });
        }
        let additional = desired_extra.min(available);
        let new_pages = current_pages
            .checked_add(additional)
            .ok_or(AssetArenaError::SizeOverflow)?;
        let new_bytes = u64::from(new_pages)
            .checked_mul(STORAGE_ALIGNMENT)
            .ok_or(AssetArenaError::SizeOverflow)?;
        let replacement = create_buffer(device, new_bytes)?;
        self.allocator.grow(additional)?;
        let mut encoder = device.create_command_encoder();
        encoder.copy_buffer_to_buffer(&self.buffer, 0, &replacement, 0, old_bytes);
        queue.submit(encoder);
        self.buffer = replacement;
        self.revision = self.revision.wrapping_add(1);
        self.metrics.physical_bytes = new_bytes;
        self.metrics.relocations = self.metrics.relocations.saturating_add(1);
        Ok(())
    }

    fn refresh_resident_metrics(&mut self) {
        self.metrics.resident_bytes = self.allocator.metrics().resident_bytes;
        self.metrics.peak_resident_bytes = self
            .metrics
            .peak_resident_bytes
            .max(self.metrics.resident_bytes);
    }
}

fn create_buffer<D: Device>(device: &D, size: u64) -> Result<D::Buffer, AssetArenaError> {
    Ok(device.create_buffer(&BufferDesc {
        label: "immutable asset arena",
        size,
        usage: BufferUsage::STORAGE
            .union(BufferUsage::COPY_SRC)
            .union(BufferUsage::COPY_DST),
    })?)
}

fn aligned_capacity(limit: u64) -> Result<u64, AssetArenaError> {
    let aligned = limit / STORAGE_ALIGNMENT * STORAGE_ALIGNMENT;
    if aligned < STORAGE_ALIGNMENT {
        return Err(AssetArenaError::Backpressure {
            requested_bytes: STORAGE_ALIGNMENT,
            capacity_bytes: limit,
        });
    }
    Ok(aligned)
}

fn page_count(bytes: u64) -> Result<u32, AssetArenaError> {
    let rounded = bytes
        .checked_add(STORAGE_ALIGNMENT - 1)
        .ok_or(AssetArenaError::SizeOverflow)?;
    u32::try_from(rounded / STORAGE_ALIGNMENT).map_err(|_| AssetArenaError::SizeOverflow)
}

fn binding_size(bytes: usize) -> Result<u64, AssetArenaError> {
    let value = u64::try_from(bytes).map_err(|_| AssetArenaError::SizeOverflow)?;
    Ok(value.max(std::mem::size_of::<u32>() as u64))
}

#[cfg(test)]
#[path = "asset_arena_tests.rs"]
mod tests;
