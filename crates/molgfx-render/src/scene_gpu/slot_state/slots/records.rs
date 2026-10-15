//! Slot-side bookkeeping: indirect arguments and shared-record adoption.

use super::GpuSlot;
use crate::error::RenderError;
use molgfx_gpu::{BufferDesc, BufferUsage, Device};

impl<D: Device> GpuSlot<D> {
    /// Updates whether immutable scientific data belongs to the displayed sample.
    pub(in crate::scene_gpu) fn sync_data_current(
        &mut self,
        placed: &molgfx_core::PlacedStructure,
        representation: &molgfx_core::Representation,
    ) -> Result<bool, RenderError> {
        let current = if let Some(surface) = &representation.molecular_surface {
            surface.validate_source(&placed.source)?;
            surface.validate_pose(placed).is_ok()
        } else {
            true
        };
        let changed = self.data_current != current;
        self.data_current = current;
        Ok(changed)
    }
    /// Records the counts this slot draws from its shared record set.
    pub(super) const fn adopt_counts(&mut self, atom_count: u32, bond_count: u32) {
        self.atom_count = atom_count;
        self.bond_count = bond_count;
    }

    /// Records the arena columns this slot's colour reads.
    pub(in crate::scene_gpu) const fn adopt_color_columns(
        &mut self,
        columns: crate::scene_gpu::color_uniforms::ResolvedColumns,
    ) {
        self.color_columns = columns;
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
}
