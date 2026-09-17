//! Instance packing, uploads, and indirect draw argument maintenance.

use super::{FAST_POINT_INDEX_LIMIT, GpuSlot};
use crate::error::RenderError;
use crate::scene_gpu::buffers::{
    CullCountInput, count, ensure_indices, upload_grow, write_args, write_counts, write_draw_args,
};
use crate::scene_gpu::quality_acceleration::hierarchy_counts;
use crate::scene_gpu::slot_types::RecordUpload;
use crate::scene_gpu::surface_slot::SurfaceSync;
use crate::scene_gpu::uniforms::RepresentationUniforms;
use molgfx_core::RepresentationKind;
use molgfx_gpu::{BufferDesc, BufferUsage, Device};

impl<D: Device> GpuSlot<D> {
    fn pack_instances(input: &mut RecordUpload<'_, D>) -> Result<(), RenderError> {
        if input.representation.kind == RepresentationKind::Beads {
            molgfx_geometry::pack_residue_beads(
                &input.placed.atoms,
                &input.placed.hierarchy,
                input.placed.secondary_structure.values(),
                input.color_property,
                input.representation,
                input.selection,
                input.atoms,
            )?;
            molgfx_geometry::build_compaction_map(
                input.atoms,
                input.placed.atoms.len(),
                input.compaction,
            )?;
            input.bonds.clear();
        } else {
            molgfx_geometry::pack_atoms_with_properties(
                &input.placed.atoms,
                &input.placed.hierarchy,
                input.placed.secondary_structure.values(),
                molgfx_geometry::PropertyColumns {
                    color: input.color_property,
                    appearance: input.appearance_property,
                },
                input.representation,
                input.selection,
                input.atoms,
            )?;
            molgfx_geometry::build_compaction_map(
                input.atoms,
                input.placed.atoms.len(),
                input.compaction,
            )?;
            molgfx_geometry::pack_bonds(
                input.placed,
                input.representation,
                input.compaction,
                input.bonds,
            )?;
        }
        Ok(())
    }

    pub(super) fn upload_records(
        &mut self,
        input: &mut RecordUpload<'_, D>,
    ) -> Result<(), RenderError> {
        Self::pack_instances(input)?;
        self.quality_acceleration.sync_topology(
            input.device,
            input.queue,
            input.atoms,
            input.bonds,
            input.placed,
            input.ray_query_layout,
        )?;
        let atom_hierarchy = hierarchy_counts(input.placed.render_bvh()?)?;
        let bond_hierarchy = self.quality_acceleration.counts();
        self.atom_count = count(input.atoms.len());
        self.bond_count = count(input.bonds.len());
        upload_grow(
            input.device,
            input.queue,
            "atom instances",
            input.atoms,
            &mut self.atoms,
            &mut self.atoms_capacity,
        )?;
        upload_grow(
            input.device,
            input.queue,
            "bond instances",
            input.bonds,
            &mut self.bonds,
            &mut self.bonds_capacity,
        )?;
        upload_grow(
            input.device,
            input.queue,
            "source-to-compacted atom indices",
            input.compaction,
            &mut self.compaction,
            &mut self.compaction_capacity,
        )?;
        if self.representation_uniforms.is_none() {
            self.representation_uniforms = Some(input.device.create_buffer(&BufferDesc {
                label: "representation uniforms",
                size: std::mem::size_of::<RepresentationUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        self.sync_surface_resources_from_upload(input)?;
        ensure_indices(
            input.device,
            "visible atom indices",
            self.atom_count.saturating_mul(2),
            &mut self.visible_atoms,
            &mut self.visible_atoms_capacity,
        )?;
        ensure_indices(
            input.device,
            "visible bond indices",
            self.bond_count,
            &mut self.visible_bonds,
            &mut self.visible_bonds_capacity,
        )?;
        self.write_args(input.device, input.queue)?;
        write_counts(
            input.device,
            input.queue,
            CullCountInput {
                atoms: self.atom_count,
                bonds: self.bond_count,
                lod_mode: if self.atom_count < 131_072 {
                    0
                } else if self.kind == RepresentationKind::Points
                    && self.bond_count == 0
                    && self.atom_count < FAST_POINT_INDEX_LIMIT
                {
                    2
                } else {
                    1
                },
                bond_break_length: input.placed.bond_break_length(),
                visual_enabled: input.representation.visual.is_some(),
                atom_bvh_nodes: atom_hierarchy.nodes,
                atom_bvh_indices: atom_hierarchy.indices,
                bond_bvh_nodes: bond_hierarchy.nodes,
                bond_bvh_indices: bond_hierarchy.indices,
            },
            &mut self.counts,
        )?;
        self.bind_record_groups(input);
        Ok(())
    }

    pub(super) fn upload_bonds(
        &mut self,
        input: &mut RecordUpload<'_, D>,
    ) -> Result<(), RenderError> {
        molgfx_geometry::build_selection_compaction(
            input.selection,
            input.placed.atoms.len(),
            input.compaction,
        )?;
        molgfx_geometry::pack_bonds(
            input.placed,
            input.representation,
            input.compaction,
            input.bonds,
        )?;
        self.quality_acceleration.sync_topology(
            input.device,
            input.queue,
            input.atoms,
            input.bonds,
            input.placed,
            input.ray_query_layout,
        )?;
        let atom_hierarchy = hierarchy_counts(input.placed.render_bvh()?)?;
        let bond_hierarchy = self.quality_acceleration.counts();
        self.bond_count = count(input.bonds.len());
        upload_grow(
            input.device,
            input.queue,
            "bond instances",
            input.bonds,
            &mut self.bonds,
            &mut self.bonds_capacity,
        )?;
        ensure_indices(
            input.device,
            "visible bond indices",
            self.bond_count,
            &mut self.visible_bonds,
            &mut self.visible_bonds_capacity,
        )?;
        self.write_args(input.device, input.queue)?;
        write_counts(
            input.device,
            input.queue,
            CullCountInput {
                atoms: self.atom_count,
                bonds: self.bond_count,
                lod_mode: u32::from(self.atom_count >= 131_072),
                bond_break_length: input.placed.bond_break_length(),
                visual_enabled: input.representation.visual.is_some(),
                atom_bvh_nodes: atom_hierarchy.nodes,
                atom_bvh_indices: atom_hierarchy.indices,
                bond_bvh_nodes: bond_hierarchy.nodes,
                bond_bvh_indices: bond_hierarchy.indices,
            },
            &mut self.counts,
        )?;
        self.bind_record_groups(input);
        Ok(())
    }

    fn bind_record_groups(&mut self, input: &RecordUpload<'_, D>) {
        self.bind(&super::bindings::RepresentationBinding {
            device: input.device,
            layout: input.layout,
            quality_layout: input.quality_layout,
            structure: input.structure_gpu,
            asset_arena: input.asset_arena,
            surface_field_fallback: input.surface_field_fallback,
            surface_normal_fallback: input.surface_normal_fallback,
            overlay: input.overlay_view,
            visual_programs: input.visual_program_buffer,
            visual_parameters: input.visual_parameter_buffer,
            visual_properties: input.visual_property_buffer,
        });
        self.bind_cull(&super::bindings::CullBinding {
            device: input.device,
            atom_layout: input.atom_cull_layout,
            bond_layout: input.bond_cull_layout,
            visual_layout: input.visual_cull_layout,
            structure: input.structure_gpu,
            asset_arena: input.asset_arena,
            frame: input.frame,
            tiles: input.cull_tiles,
            visual_programs: input.visual_program_buffer,
            visual_parameters: input.visual_parameter_buffer,
            visual_properties: input.visual_property_buffer,
        });
    }

    fn write_args(&mut self, device: &D, queue: &D::Queue) -> Result<(), RenderError> {
        write_args(device, queue, "atom draw arguments", &mut self.atom_args)?;
        write_args(device, queue, "bond draw arguments", &mut self.bond_args)?;
        write_draw_args(
            device,
            queue,
            "surface draw arguments",
            6,
            u32::from(self.kind == RepresentationKind::Surface && self.atom_count > 0),
            &mut self.surface_args,
        )
    }

    fn sync_surface_resources_from_upload(
        &mut self,
        input: &RecordUpload<'_, D>,
    ) -> Result<(), RenderError> {
        let (Some(atoms), Some(compaction), Some(uniforms)) =
            (&self.atoms, &self.compaction, &self.representation_uniforms)
        else {
            return Ok(());
        };
        self.surface.sync(&SurfaceSync {
            device: input.device,
            queue: input.queue,
            output_layout: input.surface_field_output_layout,
            input_layout: input.surface_field_input_layout,
            erosion_layout: input.surface_field_erosion_layout,
            normal_layout: input.surface_field_normal_layout,
            component_layout: input.surface_component_layout,
            structure: input.structure_gpu,
            asset_arena: input.asset_arena,
            representation: input.representation,
            atoms,
            compaction,
            uniforms,
            atom_count: self.atom_count,
            selection_bounds: input.selection_bounds,
            force_generate: true,
            quality: input.quality,
            overlay_volume: input.overlay_volume,
        })
    }
}
