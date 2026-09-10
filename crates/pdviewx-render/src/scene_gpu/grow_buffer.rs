//! A GPU buffer that grows and is never shrunk.
//!
//! Every persistent scene table needs the same three things: a buffer that may
//! not exist yet, the capacity it was created with, and the knowledge of
//! whether the last write replaced it — because a replaced buffer invalidates
//! every bind group pointing at it. Carrying those as three loose pieces per
//! table meant each one recomputed the reallocation test by hand, and getting
//! that test wrong leaves a bind group aimed at a freed allocation.
//!
//! Growth is to the next power of two so a table that fills gradually
//! reallocates a logarithmic number of times rather than once per frame, and
//! capacity is never given back: a scene that was once large is likely to be
//! large again, and the frame loop must not be paying for allocation.

use crate::error::RenderError;
use pdviewx_gpu::{BufferDesc, BufferUsage, Device, Queue};

/// The smallest allocation worth making. Below this, a buffer costs the same
/// as a larger one and reallocating it costs more than the space saved.
const MINIMUM_BYTES: u64 = 256;

/// A grow-only device buffer and the capacity it currently holds.
#[derive(Debug)]
pub(super) struct GrowBuffer<D: Device> {
    buffer: Option<D::Buffer>,
    capacity: u64,
}

impl<D: Device> Default for GrowBuffer<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: Device> GrowBuffer<D> {
    /// An empty buffer that has not been allocated yet.
    pub(super) const fn new() -> Self {
        Self {
            buffer: None,
            capacity: 0,
        }
    }

    /// The device buffer, once something has been written to it.
    pub(super) const fn get(&self) -> Option<&D::Buffer> {
        self.buffer.as_ref()
    }

    /// Writes `records`, growing first if they do not fit.
    ///
    /// Returns `true` when the underlying allocation was replaced, which is
    /// exactly when every bind group referencing this buffer must be rebuilt.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] when the records exceed the device's storage
    /// limit or the allocation fails.
    pub(super) fn upload<T: bytemuck::Pod>(
        &mut self,
        device: &D,
        queue: &D::Queue,
        label: &'static str,
        records: &[T],
    ) -> Result<bool, RenderError> {
        let bytes = bytemuck::cast_slice(records);
        let rebound = self.reserve(device, label, bytes.len() as u64)?;
        if let Some(buffer) = &self.buffer {
            queue.write_buffer(buffer, 0, bytes);
        }
        Ok(rebound)
    }

    /// Grows to hold at least `needed` bytes without writing anything.
    ///
    /// This is the streaming counterpart to [`Self::upload`]: a caller that
    /// fills the table in bounded chunks reserves the whole thing once here and
    /// then writes into it, rather than staging a full copy host-side.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] when `needed` exceeds the device's storage limit
    /// or the allocation fails.
    pub(super) fn reserve(
        &mut self,
        device: &D,
        label: &'static str,
        needed: u64,
    ) -> Result<bool, RenderError> {
        if self.buffer.is_some() && needed <= self.capacity {
            return Ok(false);
        }
        self.allocate(device, label, needed, Self::storage_usage())
    }

    fn allocate(
        &mut self,
        device: &D,
        label: &'static str,
        needed: u64,
        usage: BufferUsage,
    ) -> Result<bool, RenderError> {
        self.capacity = grow_capacity(
            needed,
            device.capabilities().max_storage_buffer_bytes,
            label,
        )?;
        self.buffer = Some(device.create_buffer(&BufferDesc {
            label,
            size: self.capacity,
            usage,
        })?);
        Ok(true)
    }

    fn storage_usage() -> BufferUsage {
        BufferUsage::STORAGE.union(BufferUsage::COPY_DST)
    }
}

/// Rounds a requirement up to the next power of two within the device's limit.
fn grow_capacity(needed: u64, limit: u64, label: &'static str) -> Result<u64, RenderError> {
    if needed > limit || limit == 0 {
        return Err(pdviewx_gpu::GpuError::LimitExceeded {
            resource: label,
            limit,
        }
        .into());
    }
    let grown = match needed.checked_next_power_of_two() {
        Some(value) => value,
        None => needed,
    };
    Ok(grown.max(MINIMUM_BYTES).min(limit))
}

#[cfg(test)]
#[path = "grow_buffer_tests.rs"]
mod tests;
