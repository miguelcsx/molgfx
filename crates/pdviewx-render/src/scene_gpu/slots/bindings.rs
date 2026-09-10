//! Persistent bind groups for one molecular representation slot.

use super::GpuSlot;
use crate::scene_gpu::asset_arena::AssetArena;
use crate::scene_gpu::buffers::buffer_entry;
use crate::scene_gpu::structure::GpuStructure;
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, Device};

pub(super) struct CullBinding<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) atom_layout: &'a D::BindGroupLayout,
    pub(super) bond_layout: &'a D::BindGroupLayout,
    pub(super) visual_layout: &'a D::BindGroupLayout,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) frame: &'a D::Buffer,
    pub(super) tiles: &'a D::Buffer,
    pub(super) visual_programs: Option<&'a D::Buffer>,
    pub(super) visual_parameters: Option<&'a D::Buffer>,
    pub(super) visual_properties: Option<&'a D::Buffer>,
}

pub(super) struct RepresentationBinding<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) quality_layout: &'a D::BindGroupLayout,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) surface_field_fallback: &'a D::TextureView,
    pub(super) surface_normal_fallback: &'a D::TextureView,
    pub(super) overlay: &'a D::TextureView,
    pub(super) visual_programs: Option<&'a D::Buffer>,
    pub(super) visual_parameters: Option<&'a D::Buffer>,
    pub(super) visual_properties: Option<&'a D::Buffer>,
}

impl<D: Device> GpuSlot<D> {
    pub(super) fn bind_cull(&mut self, input: &CullBinding<'_, D>) {
        let (
            Some(atoms),
            Some(bonds),
            Some(visible_atoms),
            Some(visible_bonds),
            Some(atom_args),
            Some(bond_args),
            Some(counts),
            Some(model),
            Some(visual_programs),
            Some(visual_parameters),
            Some(visual_properties),
        ) = (
            &self.atoms,
            &self.bonds,
            &self.visible_atoms,
            &self.visible_bonds,
            &self.atom_args,
            &self.bond_args,
            &self.counts,
            &input.structure.model,
            input.visual_programs,
            input.visual_parameters,
            input.visual_properties,
        )
        else {
            return;
        };
        let Some(visual) =
            self.visual
                .cull_entries(visual_programs, visual_parameters, visual_properties)
        else {
            return;
        };
        self.atom_cull_group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "atom cull slot",
            layout: input.atom_layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(2, visible_atoms),
                buffer_entry(4, atom_args),
                buffer_entry(5, bond_args),
                buffer_entry(6, counts),
                buffer_entry(7, input.frame),
                buffer_entry(8, model),
                input.structure.coords_entry(input.asset_arena, 9),
                buffer_entry(10, input.tiles),
                buffer_entry(14, visual.results),
                buffer_entry(15, visual.config),
            ],
        }));
        self.bond_cull_group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "bond cull slot",
            layout: input.bond_layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(1, bonds),
                buffer_entry(2, visible_atoms),
                buffer_entry(3, visible_bonds),
                buffer_entry(4, atom_args),
                buffer_entry(5, bond_args),
                buffer_entry(6, counts),
                buffer_entry(7, input.frame),
                buffer_entry(8, model),
                input.structure.coords_entry(input.asset_arena, 9),
                buffer_entry(14, visual.results),
                buffer_entry(15, visual.config),
            ],
        }));
        self.visual_cull_group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "visual cull slot",
            layout: input.visual_layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(2, visible_atoms),
                buffer_entry(4, atom_args),
                buffer_entry(8, model),
                input.structure.coords_entry(input.asset_arena, 9),
                buffer_entry(11, visual.instructions),
                buffer_entry(12, visual.parameters),
                buffer_entry(13, visual.properties),
                buffer_entry(14, visual.results),
                buffer_entry(15, visual.config),
            ],
        }));
    }

    pub(super) fn bind(&mut self, input: &RepresentationBinding<'_, D>) {
        let (
            Some(atoms),
            Some(model),
            Some(bonds),
            Some(visible_atoms),
            Some(visible_bonds),
            Some(compaction),
            Some(representation_uniforms),
            Some(counts),
        ) = (
            &self.atoms,
            &input.structure.model,
            &self.bonds,
            &self.visible_atoms,
            &self.visible_bonds,
            &self.compaction,
            &self.representation_uniforms,
            &self.counts,
        )
        else {
            return;
        };
        let surface_grid = self.surface.field_binding(input.surface_field_fallback);
        let surface_normals = self.surface.normal_binding(input.surface_normal_fallback);
        let (Some(visual_programs), Some(visual_parameters), Some(visual_properties)) = (
            input.visual_programs,
            input.visual_parameters,
            input.visual_properties,
        ) else {
            return;
        };
        let Some(visual) =
            self.visual
                .cull_entries(visual_programs, visual_parameters, visual_properties)
        else {
            return;
        };
        let quality_data = match self.quality_acceleration.nodes() {
            Some(nodes) => buffer_entry(21, nodes),
            None => input.structure.bvh_nodes_entry(input.asset_arena, 21),
        };
        self.group2 = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "group2: per-representation",
            layout: input.layout,
            entries: &[
                buffer_entry(0, atoms),
                input.structure.coords_entry(input.asset_arena, 1),
                buffer_entry(2, model),
                buffer_entry(3, bonds),
                buffer_entry(4, visible_atoms),
                buffer_entry(5, visible_bonds),
                input.structure.bvh_nodes_entry(input.asset_arena, 6),
                input.structure.bvh_indices_entry(input.asset_arena, 7),
                buffer_entry(8, compaction),
                buffer_entry(9, representation_uniforms),
                BindGroupEntry::Texture {
                    binding: 10,
                    view: surface_grid,
                },
                BindGroupEntry::Texture {
                    binding: 11,
                    view: surface_normals,
                },
                BindGroupEntry::Texture {
                    binding: 12,
                    view: input.overlay,
                },
                input.structure.previous_coords_entry(input.asset_arena, 13),
                buffer_entry(14, counts),
                buffer_entry(16, visual.results),
                buffer_entry(17, visual.fragment_program),
                buffer_entry(19, visual.properties),
                buffer_entry(20, visual.config),
            ],
        }));
        self.quality_group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "group2: quality tracing",
            layout: input.quality_layout,
            entries: &[
                buffer_entry(0, atoms),
                input.structure.coords_entry(input.asset_arena, 1),
                buffer_entry(2, model),
                input.structure.bvh_nodes_entry(input.asset_arena, 6),
                input.structure.bvh_indices_entry(input.asset_arena, 7),
                buffer_entry(8, compaction),
                buffer_entry(9, representation_uniforms),
                buffer_entry(14, counts),
                buffer_entry(16, visual.results),
                buffer_entry(17, visual.fragment_program),
                buffer_entry(19, visual.properties),
                buffer_entry(20, visual.config),
                quality_data,
            ],
        }));
    }
}
