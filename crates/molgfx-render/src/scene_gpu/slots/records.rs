//! Slot-side bookkeeping: indirect arguments and shared-record adoption.

use super::GpuSlot;
use crate::error::RenderError;
use molgfx_gpu::{BufferDesc, BufferUsage, Device};

impl<D: Device> GpuSlot<D> {
    /// Records the counts this slot draws from its shared record set.
    pub(super) const fn adopt_counts(&mut self, atom_count: u32, bond_count: u32) {
        self.atom_count = atom_count;
        self.bond_count = bond_count;
    }

    /// Records the colour property column's arena offset and stride.
    pub(in crate::scene_gpu) const fn adopt_color_column(&mut self, column: [u32; 2]) {
        self.color_column = column;
    }

    /// Records the overlay class column's arena offset and stride.
    pub(in crate::scene_gpu) const fn adopt_overlay_column(&mut self, column: [u32; 2]) {
        self.overlay_column = column;
    }

    /// Reserves the per-representation uniform buffer the first time it syncs.
    pub(super) fn ensure_uniforms(&mut self, device: &D) -> Result<(), RenderError> {
        if self.representation_uniforms.is_none() {
            self.representation_uniforms = Some(device.create_buffer(&BufferDesc {
                label: "representation uniforms",
                size: std::mem::size_of::<crate::scene_gpu::uniforms::RepresentationUniforms>()
                    as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        if self.color_uniforms.is_none() {
            self.color_uniforms = Some(device.create_buffer(&BufferDesc {
                label: "colour scheme uniforms",
                size: std::mem::size_of::<crate::scene_gpu::color_uniforms::ColorUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        Ok(())
    }

    /// The level-of-detail mode the cull shader reads for this slot.
    pub(crate) const fn lod_mode(&self) -> u32 {
        crate::scene_gpu::slot_types::lod_mode(self.atom_count, self.bond_count, self.kind)
    }
}
