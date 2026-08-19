//! Persistent bind groups for one molecular representation slot.

use super::GpuSlot;
use crate::scene_gpu::buffers::buffer_entry;
use crate::scene_gpu::structure::GpuStructure;
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, Device};

impl<D: Device> GpuSlot<D> {
    pub(super) fn bind_cull(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        structure: &GpuStructure<D>,
        frame: &D::Buffer,
        cull_tiles: &D::Buffer,
    ) {
        let (
            Some(atoms),
            Some(bonds),
            Some(visible_atoms),
            Some(visible_bonds),
            Some(atom_args),
            Some(bond_args),
            Some(counts),
            Some(coords),
            Some(model),
        ) = (
            &self.atoms,
            &self.bonds,
            &self.visible_atoms,
            &self.visible_bonds,
            &self.atom_args,
            &self.bond_args,
            &self.counts,
            structure.coords(),
            &structure.model,
        )
        else {
            return;
        };
        self.cull_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "cull slot",
            layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(1, bonds),
                buffer_entry(2, visible_atoms),
                buffer_entry(3, visible_bonds),
                buffer_entry(4, atom_args),
                buffer_entry(5, bond_args),
                buffer_entry(6, counts),
                buffer_entry(7, frame),
                buffer_entry(8, model),
                buffer_entry(9, coords),
                buffer_entry(10, cull_tiles),
            ],
        }));
    }

    pub(super) fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        structure: &GpuStructure<D>,
        surface_field_fallback: &D::TextureView,
        surface_provenance_fallback: &D::TextureView,
        overlay_view: &D::TextureView,
    ) {
        let (
            Some(atoms),
            Some(coords),
            Some(previous_coords),
            Some(model),
            Some(bonds),
            Some(visible_atoms),
            Some(visible_bonds),
            Some(bvh_nodes),
            Some(bvh_indices),
            Some(bvh_escape),
            Some(compaction),
            Some(representation_uniforms),
            Some(counts),
        ) = (
            &self.atoms,
            structure.coords(),
            structure.previous_coords(),
            &structure.model,
            &self.bonds,
            &self.visible_atoms,
            &self.visible_bonds,
            &structure.bvh_nodes,
            &structure.bvh_indices,
            &structure.bvh_escape,
            &self.compaction,
            &self.representation_uniforms,
            &self.counts,
        )
        else {
            return;
        };
        let surface_grid = self.surface.field_binding(surface_field_fallback);
        let surface_provenance = self.surface.provenance_binding(surface_provenance_fallback);
        self.group2 = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: per-representation",
            layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(1, coords),
                buffer_entry(2, model),
                buffer_entry(3, bonds),
                buffer_entry(4, visible_atoms),
                buffer_entry(5, visible_bonds),
                buffer_entry(6, bvh_nodes),
                buffer_entry(7, bvh_indices),
                buffer_entry(8, compaction),
                buffer_entry(9, representation_uniforms),
                BindGroupEntry::Texture {
                    binding: 10,
                    view: surface_grid,
                },
                BindGroupEntry::Texture {
                    binding: 11,
                    view: surface_provenance,
                },
                BindGroupEntry::Texture {
                    binding: 12,
                    view: overlay_view,
                },
                buffer_entry(13, previous_coords),
                buffer_entry(14, counts),
                buffer_entry(15, bvh_escape),
            ],
        }));
    }
}
