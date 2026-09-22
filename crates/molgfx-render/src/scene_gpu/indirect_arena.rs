//! One scene-wide indirect-argument arena.
//!
//! Every draw reads its arguments from the same buffer, at the aligned slot its
//! key owns. Three small buffers per representation become one allocation for
//! the whole scene, which is what the portable buffer budget cares about.
//!
//! A slot's shader sees the arena through a [`BindGroupEntry::BufferRange`]
//! slice starting at the slot, so culling writes instance counts at "offset
//! zero" of its own view, and the draw passes that same real offset to
//! `draw_indirect`. No draw needs a first-instance offset.

use super::record_cache::RecordKey;
use super::visibility_cache::VisibilityKey;
use crate::error::RenderError;
use molgfx_core::DrawIndirectArgs;
use molgfx_gpu::{BufferDesc, BufferUsage, Device, Queue};

/// Bytes per slot.
///
/// One `DrawIndirectArgs` is 16 bytes, but a slot is also bound as a ranged
/// storage buffer so the cull shader can write its own counts, and a ranged
/// binding's offset must satisfy the device's storage-buffer offset alignment.
/// Padding every slot to the portable 256-byte alignment is what lets one arena
/// serve every draw without a per-slot copy.
pub(super) const SLOT_STRIDE: u64 = 256;

/// A non-molecular drawable's arguments, keyed by the geometry it draws.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum IndirectSlotKey {
    /// Atoms of one shared visible set.
    Atom(VisibilityKey),
    /// Bonds of one shared visible set.
    Bond(VisibilityKey),
    /// The surface mesh of one shared record set.
    Surface(RecordKey),
}

/// Grow-only arena of aligned indirect-argument slots.
#[derive(Debug)]
pub(super) struct IndirectArgsArena<D: Device> {
    buffer: Option<D::Buffer>,
    capacity: u64,
    /// Slot index per key, assigned on first use and released when unused.
    slots: Vec<(IndirectSlotKey, u32)>,
    /// The arguments last written per key, so an unchanged frame writes none.
    written: Vec<(IndirectSlotKey, (u32, u32))>,
}

impl<D: Device> IndirectArgsArena<D> {
    pub(super) const fn new() -> Self {
        Self {
            buffer: None,
            capacity: 0,
            slots: Vec::new(),
            written: Vec::new(),
        }
    }

    /// The byte offset of `key`'s slot, allocating one if it is new.
    fn slot(&mut self, device: &D, key: IndirectSlotKey) -> Result<u64, RenderError> {
        let index =
            if let Some((_, index)) = self.slots.iter().find(|(candidate, _)| *candidate == key) {
                *index
            } else {
                // Slots are never recycled within a frame, so a released key's
                // index can only be reused after the frame's writes are gone.
                let index = u32::try_from(self.slots.len()).map_err(|_| {
                    molgfx_gpu::GpuError::LimitExceeded {
                        resource: "indirect argument slots",
                        limit: u64::from(u32::MAX),
                    }
                })?;
                self.slots.push((key, index));
                index
            };
        let needed = SLOT_STRIDE.saturating_mul(u64::from(index) + 1);
        if self.buffer.is_none() || needed > self.capacity {
            // Sized with headroom for a scene's initial slots: growing one slot
            // at a time would allocate a new arena per representation, which is
            // the per-slot allocation the single arena exists to avoid.
            self.capacity = needed.next_power_of_two().max(SLOT_STRIDE * 16);
            self.buffer = Some(
                device.create_buffer(&BufferDesc {
                    label: "indirect draw arguments",
                    size: self.capacity,
                    usage: BufferUsage::INDIRECT
                        .union(BufferUsage::STORAGE)
                        .union(BufferUsage::COPY_DST),
                })?,
            );
        }
        Ok(SLOT_STRIDE.saturating_mul(u64::from(index)))
    }

    /// Writes one draw's arguments into its slot.
    pub(super) fn write(
        &mut self,
        device: &D,
        queue: &D::Queue,
        key: IndirectSlotKey,
        vertex_count: u32,
        instance_count: u32,
    ) -> Result<u64, RenderError> {
        let offset = self.slot(device, key)?;
        let arguments = (vertex_count, instance_count);
        // A stable frame must upload nothing but its own uniforms, so a slot is
        // rewritten only when its arguments actually change.
        match self
            .written
            .iter()
            .position(|(candidate, _)| *candidate == key)
        {
            Some(index) if self.written[index].1 == arguments => return Ok(offset),
            Some(index) => self.written[index].1 = arguments,
            None => self.written.push((key, arguments)),
        }
        let Some(buffer) = self.buffer.as_ref() else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        queue.write_buffer(
            buffer,
            offset,
            bytemuck::bytes_of(&DrawIndirectArgs {
                vertex_count,
                instance_count,
                first_vertex: 0,
                first_instance: 0,
            }),
        );
        Ok(offset)
    }

    /// Releases every slot no draw claimed this frame.
    pub(super) fn retain(&mut self, live: &[IndirectSlotKey]) {
        self.slots.retain(|(key, _)| live.contains(key));
        self.written.retain(|(key, _)| live.contains(key));
    }

    /// The arena buffer, bound as a whole.
    #[must_use]
    pub(super) fn buffer(&self) -> Option<&D::Buffer> {
        self.buffer.as_ref()
    }

    /// Resident device bytes.
    #[must_use]
    pub(crate) fn resident_bytes(&self) -> u64 {
        self.capacity
    }
}
