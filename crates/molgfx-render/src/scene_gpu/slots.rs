//! Persistent structure allocations and drawable representation slots.

mod bindings;
mod draws;
mod records;
mod state;
use super::quality_acceleration::QualityAcceleration;
use super::ribbon_slot::{RibbonSlot, RibbonSync};
use super::slot_types::{SlotKey, SlotPlan, SlotShading, SlotSync, SlotSynced};
use super::surface_slot::{SurfaceSlot, SurfaceSync};
use super::sync::selection_bounds::SelectionBoundsCache;
use super::uniforms::write_representation_uniforms;
use super::visual::{VisualBase, VisualSlot, VisualSync};
use crate::error::RenderError;
use bindings::{CullBinding, RepresentationBinding};
use molgfx_core::RepresentationKind;
use molgfx_gpu::Device;
use state::{is_spline, shading, synced_state};

const FAST_POINT_INDEX_LIMIT: u32 = 1 << 24;
#[derive(Debug)]
pub(super) struct GpuSlot<D: Device> {
    pub(super) key: SlotKey,
    pub(super) structure_index: usize,
    atoms: Option<D::Buffer>,
    bonds: Option<D::Buffer>,
    atom_args: Option<D::Buffer>,
    bond_args: Option<D::Buffer>,
    surface_args: Option<D::Buffer>,
    compaction: Option<D::Buffer>,
    representation_uniforms: Option<D::Buffer>,
    surface: SurfaceSlot<D>,
    group2: Option<D::BindGroup>,
    quality_group: Option<D::BindGroup>,
    atom_cull_group: Option<D::BindGroup>,
    bond_cull_group: Option<D::BindGroup>,
    visual_cull_group: Option<D::BindGroup>,
    visible_atoms: Option<D::Buffer>,
    visible_bonds: Option<D::Buffer>,
    counts: Option<D::Buffer>,
    atom_count: u32,
    bond_count: u32,
    atoms_capacity: u64,
    bonds_capacity: u64,
    compaction_capacity: u64,
    visible_atoms_capacity: u64,
    visible_bonds_capacity: u64,
    selection_bounds: SelectionBoundsCache,
    synced: Option<SlotSynced>,
    translucent: bool,
    kind: RepresentationKind,
    shading: SlotShading,
    quality_acceleration: QualityAcceleration<D>,
    ribbon: RibbonSlot<D>,
    visual: VisualSlot<D>,
}
impl<D: Device> GpuSlot<D> {
    pub(super) fn new(plan: SlotPlan) -> Self {
        Self {
            key: plan.key,
            structure_index: plan.structure_index,
            atoms: None,
            bonds: None,
            atom_args: None,
            bond_args: None,
            surface_args: None,
            compaction: None,
            representation_uniforms: None,
            surface: SurfaceSlot::new(),
            group2: None,
            quality_group: None,
            atom_cull_group: None,
            bond_cull_group: None,
            visual_cull_group: None,
            visible_atoms: None,
            visible_bonds: None,
            counts: None,
            atom_count: 0,
            bond_count: 0,
            atoms_capacity: 0,
            bonds_capacity: 0,
            compaction_capacity: 0,
            visible_atoms_capacity: 0,
            visible_bonds_capacity: 0,
            selection_bounds: SelectionBoundsCache::new(),
            synced: None,
            translucent: false,
            shading: SlotShading::default(),
            quality_acceleration: QualityAcceleration::new(),
            kind: RepresentationKind::Spacefill,
            ribbon: RibbonSlot::new(),
            visual: VisualSlot::new(),
        }
    }

    pub(super) fn sync(&mut self, mut input: SlotSync<'_, D>) -> Result<bool, RenderError> {
        let current = synced_state(&input);
        if self.synced == Some(current) {
            return Ok(false);
        }
        let selection_bounds =
            self.selection_bounds
                .resolve(input.placed, input.representation, input.selection)?;
        self.translucent = input.representation.is_translucent();
        self.kind = input.representation.kind;
        self.shading = shading(input.representation);
        let representation_changed = self
            .synced
            .is_none_or(|old| old.presentation != current.presentation);
        let records_changed = self.synced.is_none_or(|old| {
            old.records != current.records
                || old.color != current.color
                || old.flags != current.flags
                || old.semantic != current.semantic
                || old.properties != current.properties
        });
        let topology_changed = self
            .synced
            .is_none_or(|old| old.bond_topology != current.bond_topology);
        let spatial_bounds_changed = self
            .synced
            .is_none_or(|old| old.spatial_bounds != current.spatial_bounds);
        let placement_changed = self
            .synced
            .is_none_or(|old| old.placement_transform != current.placement_transform);
        let ribbon_binding_changed = self.sync_drawable_resources(
            &mut input,
            &current,
            selection_bounds,
            representation_changed,
            records_changed,
            topology_changed,
        )?;
        if spatial_bounds_changed && !records_changed && !topology_changed && !is_spline(self.kind)
        {
            self.quality_acceleration.sync_coordinates(
                input.device,
                input.queue,
                input.placed,
                input.atoms,
                input.bonds,
                input.ray_query_layout,
            )?;
            self.bind_representation(&input);
        }
        if placement_changed && !spatial_bounds_changed {
            self.quality_acceleration
                .sync_placement(input.device, input.placed);
        }
        if self.cull_binding_changed(&current) {
            self.bind_cull_input(&input);
        }
        let visual_changed = self.sync_visual(&input)?;
        self.refresh_visual_bindings(&input, &current, ribbon_binding_changed, visual_changed);
        self.sync_uniforms(&input, &current, selection_bounds, representation_changed);
        self.synced = Some(current);
        Ok(true)
    }

    fn sync_drawable_resources(
        &mut self,
        input: &mut SlotSync<'_, D>,
        current: &SlotSynced,
        selection_bounds: molgfx_math::Aabb,
        representation_changed: bool,
        records_changed: bool,
        topology_changed: bool,
    ) -> Result<bool, RenderError> {
        let binding_changed = if is_spline(input.representation.kind) {
            self.sync_cartoon(input, current, representation_changed, selection_bounds)?
        } else if records_changed {
            self.ribbon.clear();
            self.upload_records(&mut input.records(selection_bounds))?;
            false
        } else if topology_changed {
            self.upload_bonds(&mut input.records(selection_bounds))?;
            false
        } else if self.kind == RepresentationKind::Surface
            && (representation_changed
                || self.synced.is_none_or(|old| {
                    old.quality != current.quality
                        || old.coordinates != current.coordinates
                        || old.overlay_binding != current.overlay_binding
                }))
        {
            let coordinates_changed = self
                .synced
                .is_none_or(|old| old.coordinates != current.coordinates);
            let quality_changed = self.synced.is_none_or(|old| old.quality != current.quality);
            self.sync_surface_resources(
                input,
                selection_bounds,
                coordinates_changed || quality_changed,
            )?;
            self.bind_representation(input);
            false
        } else if self.group2.is_none()
            || self.synced.is_none_or(|old| {
                old.structure_binding != current.structure_binding
                    || old.overlay_binding != current.overlay_binding
                    || old.visual_program_binding != current.visual_program_binding
                    || old.visual_parameter_binding != current.visual_parameter_binding
                    || old.visual_property_binding != current.visual_property_binding
            })
        {
            self.bind_representation(input);
            false
        } else {
            false
        };
        Ok(binding_changed)
    }

    fn refresh_visual_bindings(
        &mut self,
        input: &SlotSync<'_, D>,
        current: &SlotSynced,
        ribbon_binding_changed: bool,
        visual_changed: bool,
    ) {
        if is_spline(self.kind) {
            let visual_binding_changed = self.synced.is_none_or(|old| {
                old.visual_program_binding != current.visual_program_binding
                    || old.visual_parameter_binding != current.visual_parameter_binding
                    || old.visual_property_binding != current.visual_property_binding
            });
            if ribbon_binding_changed || visual_changed || visual_binding_changed {
                self.bind_ribbon(input);
            }
        } else if visual_changed {
            self.bind_representation(input);
            self.bind_cull_input(input);
        }
    }

    fn bind_representation(&mut self, input: &SlotSync<'_, D>) {
        self.bind(&RepresentationBinding {
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
    }

    fn bind_cull_input(&mut self, input: &SlotSync<'_, D>) {
        self.bind_cull(&CullBinding {
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

    fn bind_ribbon(&mut self, input: &SlotSync<'_, D>) {
        let (Some(instructions), Some(parameters), Some(properties)) = (
            input.visual_program_buffer,
            input.visual_parameter_buffer,
            input.visual_property_buffer,
        ) else {
            return;
        };
        let visual = self
            .visual
            .cull_entries(instructions, parameters, properties);
        self.ribbon.bind(
            input.device,
            input.ribbon_layout,
            input.structure_gpu,
            input.asset_arena,
            visual,
        );
    }

    fn sync_visual(&mut self, input: &SlotSync<'_, D>) -> Result<bool, RenderError> {
        let Some(parameter_buffer) = input.visual_parameter_buffer else {
            return Err(molgfx_gpu::GpuError::LimitExceeded {
                resource: "visual parameter arena",
                limit: 0,
            }
            .into());
        };
        self.visual.sync(&VisualSync {
            device: input.device,
            queue: input.queue,
            style: input.representation.visual.as_ref(),
            program_offset: input.visual_program_offset,
            parameter_buffer,
            parameter_offset: input.visual_parameter_offset,
            parameters_preloaded: false,
            property_offsets: input.visual_property_offsets,
            attribute_layouts: input.visual_attribute_layouts,
            property_end_offsets: [0; 4],
            property_alphas: [0.0; 4],
            state_offset: input.visual_state_offset,
            time_seconds: input.visual_time_seconds,
            entity_count: self.atom_count as usize,
            result_count: input.placed.atoms.len() as usize,
            base: VisualBase::representation(input.representation),
        })
    }

    fn cull_binding_changed(&self, current: &SlotSynced) -> bool {
        self.atom_cull_group.is_none()
            || self.bond_cull_group.is_none()
            || self.visual_cull_group.is_none()
            || self.synced.is_some_and(|old| {
                old.structure_binding != current.structure_binding
                    || old.visual_program_binding != current.visual_program_binding
                    || old.visual_parameter_binding != current.visual_parameter_binding
                    || old.visual_property_binding != current.visual_property_binding
            })
    }

    fn sync_cartoon(
        &mut self,
        input: &mut SlotSync<'_, D>,
        current: &SlotSynced,
        representation_changed: bool,
        selection_bounds: molgfx_math::Aabb,
    ) -> Result<bool, RenderError> {
        let visual_records_changed = input.representation.visual.is_some()
            && self.synced.is_none_or(|old| {
                old.records != current.records
                    || old.color != current.color
                    || old.flags != current.flags
                    || old.semantic != current.semantic
                    || old.visual_program != current.visual_program
            });
        if visual_records_changed {
            self.upload_records(&mut input.records(selection_bounds))?;
            self.bond_count = 0;
        } else if input.representation.visual.is_none() {
            self.atom_count = 0;
            self.bond_count = 0;
        }
        let geometry_changed = self.synced.is_none_or(|old| {
            old.ribbon != current.ribbon
                || old.color != current.color
                || old.flags != current.flags
                || old.semantic != current.semantic
                || old.properties != current.properties
                || old.secondary_structure != current.secondary_structure
        });
        if geometry_changed {
            self.ribbon.sync(&mut RibbonSync {
                device: input.device,
                queue: input.queue,
                placed: input.placed,
                representation: input.representation,
                selection: input.selection,
                mesh: input.ribbon,
                color_property: input.color_property,
                appearance_property: input.appearance_property,
            })?;
        } else {
            if representation_changed {
                self.ribbon
                    .sync_clipping(input.device, input.queue, input.representation)?;
            }
            if self
                .synced
                .is_none_or(|old| old.structure_binding != current.structure_binding)
            {
                return Ok(true);
            }
        }
        Ok(geometry_changed || visual_records_changed)
    }

    fn sync_uniforms(
        &self,
        input: &SlotSync<'_, D>,
        current: &SlotSynced,
        selection_bounds: molgfx_math::Aabb,
        representation_changed: bool,
    ) {
        let resource_changed = self.synced.is_none_or(|old| {
            old.quality != current.quality
                || old.spatial_bounds != current.spatial_bounds
                || old.structure_binding != current.structure_binding
                || old.overlay_binding != current.overlay_binding
        });
        if is_spline(self.kind) || (!representation_changed && !resource_changed) {
            return;
        }
        if let Some(uniforms) = &self.representation_uniforms {
            write_representation_uniforms::<D>(
                input.queue,
                uniforms,
                input.representation,
                selection_bounds,
                input.overlay_volume,
                input.quality,
            );
        }
    }

    fn sync_surface_resources(
        &mut self,
        input: &SlotSync<'_, D>,
        selection_bounds: molgfx_math::Aabb,
        force_generate: bool,
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
            selection_bounds,
            force_generate,
            quality: input.quality,
            overlay_volume: input.overlay_volume,
        })
    }
}
