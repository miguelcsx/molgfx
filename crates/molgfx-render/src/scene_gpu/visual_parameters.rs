//! Fixed-range scene-wide visual parameter storage.
//!
//! Each resident representation owns sixteen `vec4` lanes, matching the
//! portable program limit. Offsets therefore stay stable until scene
//! composition changes, and a parameter edit is one direct subrange write with
//! no allocation or host-side table rebuild.

use super::grow_buffer::GrowBuffer;
use crate::error::RenderError;
use molgfx_core::MAX_VISUAL_PARAMETERS;
use molgfx_gpu::Device;

const PARAMETER_LANE_BYTES: u64 = std::mem::size_of::<[f32; 4]>() as u64;

#[derive(Debug)]
pub(super) struct VisualParameterTable<D: Device> {
    buffer: GrowBuffer<D>,
    binding_revision: u64,
}

impl<D: Device> VisualParameterTable<D> {
    pub(super) const fn new() -> Self {
        Self {
            buffer: GrowBuffer::new(),
            binding_revision: 0,
        }
    }

    pub(super) fn reserve(&mut self, device: &D, slots: usize) -> Result<bool, RenderError> {
        let slot_count =
            u64::try_from(slots.max(1)).map_err(|_| molgfx_gpu::GpuError::LimitExceeded {
                resource: "visual parameter slots",
                limit: u64::MAX,
            })?;
        let needed = slot_count
            .saturating_mul(MAX_VISUAL_PARAMETERS as u64)
            .saturating_mul(PARAMETER_LANE_BYTES);
        let rebound = self
            .buffer
            .reserve(device, "visual parameter arena", needed)?;
        if rebound {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        Ok(rebound)
    }

    pub(super) const fn buffer(&self) -> Option<&D::Buffer> {
        self.buffer.get()
    }

    pub(super) fn offset(slot: usize) -> u32 {
        let lanes = slot.saturating_mul(MAX_VISUAL_PARAMETERS);
        crate::fallback(u32::try_from(lanes), u32::MAX)
    }

    pub(super) const fn binding_revision(&self) -> u64 {
        self.binding_revision
    }
}

#[cfg(test)]
#[path = "visual_parameters_tests.rs"]
mod tests;
