//! Persistent structure allocations and drawable representation slots.

mod bindings;
mod draws;
mod records;
mod state;
use super::placement_acceleration::PlacementAcceleration;
use super::record_cache::RecordKey;
use super::ribbon_slot::{RibbonSlot, RibbonSync};
use super::slot_types::{
    DrawSpecializations, SlotKey, SlotPlan, SlotShading, SlotSync, SlotSynced,
};
use super::surface_slot::SurfaceSlot;
use super::sync::selection_bounds::SelectionBoundsCache;
use super::uniforms::write_representation_uniforms;
use super::visual::{VisualBase, VisualSlot, VisualSync};
use crate::error::RenderError;
use bindings::{CullBinding, RepresentationBinding};
use molgfx_core::RepresentationKind;
use molgfx_gpu::Device;
use state::{is_spline, shading, synced_state};

pub(crate) const FAST_POINT_INDEX_LIMIT: u32 = 1 << 24;

#[derive(Debug)]
pub(super) struct GpuSlot<D: Device> {
    pub(super) key: SlotKey,
    /// Which shared record set this slot draws from.
    pub(super) record_key: Option<RecordKey>,
    /// The cull policy this slot's shared visible set was computed under.
    pub(super) visibility_key: Option<super::VisibilityKey>,
    pub(super) structure_index: usize,
    pub(super) draw_order: usize,
    pub(super) visible: bool,
    atom_args: Option<u64>,
    bond_args: Option<u64>,
    surface_args: Option<u64>,
    representation_uniforms: Option<D::Buffer>,
    color_uniforms: Option<D::Buffer>,
    /// The colour property column's arena offset and stride in words.
    color_column: [u32; 2],
    /// The overlay class column's arena offset and stride.
    overlay_column: [u32; 2],
    pub(in crate::scene_gpu) surface: SurfaceSlot,
    group2: Option<D::BindGroup>,
    quality_group: Option<D::BindGroup>,
    atom_cull_group: Option<D::BindGroup>,
    bond_cull_group: Option<D::BindGroup>,
    visual_cull_group: Option<D::BindGroup>,
    pub(crate) atom_count: u32,
    pub(crate) bond_count: u32,
    pub(in crate::scene_gpu) selection_bounds: SelectionBoundsCache,
    synced: Option<SlotSynced>,
    translucent: bool,
    kind: RepresentationKind,
    shading: SlotShading,
    /// The generated pipeline each family resolved to, refreshed once per
    /// frame before any pass records. A slot without a style holds a table of
    /// `None`, so its draws keep the interpreted pipeline.
    specialized: DrawSpecializations,
    hardware: PlacementAcceleration<D>,
    ribbon: RibbonSlot<D>,
    visual: VisualSlot<D>,
}

impl<D: Device> GpuSlot<D> {
    pub(super) fn new(plan: SlotPlan) -> Self {
        Self {
            key: plan.key,
            record_key: None,
            visibility_key: None,
            structure_index: plan.structure_index,
            draw_order: plan.draw_order,
            visible: plan.visible,
            atom_args: None,
            bond_args: None,
            surface_args: None,
            representation_uniforms: None,
            color_uniforms: None,
            color_column: [0, 1],
            overlay_column: [0, 1],
            surface: SurfaceSlot::new(),
            group2: None,
            quality_group: None,
            atom_cull_group: None,
            bond_cull_group: None,
            visual_cull_group: None,
            atom_count: 0,
            bond_count: 0,
            selection_bounds: SelectionBoundsCache::new(),
            synced: None,
            translucent: false,
            shading: SlotShading::default(),
            specialized: [None; super::slot_types::DRAW_FAMILIES],
            hardware: PlacementAcceleration::new(),
            kind: RepresentationKind::Spacefill,
            ribbon: RibbonSlot::new(),
            visual: VisualSlot::new(),
        }
    }

    pub(super) fn sync(&mut self, mut input: SlotSync<'_, D>) -> Result<bool, RenderError> {
        self.visible = input.representation.visible;
        self.kind = input.representation.kind;
        let current = synced_state(&input);
        // Argument slots are scene state resolved in the record pass; adopt
        // them even on a frame that changes nothing else, because a slot may
        // be seeing its records for the first time.
        if let Some((atom, bond, surface)) = input.args {
            self.atom_args = Some(atom);
            self.bond_args = Some(bond);
            self.surface_args = Some(surface);
        }
        if self.synced == Some(current) {
            return Ok(false);
        }
        self.adopt_counts(input.records.atom_count, input.records.bond_count);
        self.ensure_uniforms(input.device)?;
        let records_changed = self.record_key != Some(current.record_key);
        let topology_changed = self
            .synced
            .is_none_or(|old| old.bond_topology != current.bond_topology);
        self.record_key = Some(current.record_key);
        let selection_bounds =
            self.selection_bounds
                .resolve(input.placed, input.representation, input.selection)?;
        self.translucent = input.representation.is_translucent();
        self.shading = shading(input.representation);
        let representation_changed = self
            .synced
            .is_none_or(|old| old.presentation != current.presentation);
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
        if placement_changed || self.synced.is_none() {
            let blas = input.acceleration_draw.and_then(|value| value.blas());
            self.hardware.sync(
                input.device,
                blas,
                input.ray_query_layout,
                input.placed.model_to_world,
            );
        }
        if spatial_bounds_changed && !records_changed && !topology_changed && !is_spline(self.kind)
        {
            self.bind_representation(&input);
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
        // Packed records are resident in the cache already, so a record or
        // topology change needs no upload here — only a surface field rebuild,
        // which the branch below covers.
        let _ = (records_changed, topology_changed);
        let binding_changed = if is_spline(input.representation.kind) {
            self.sync_cartoon(input, current, representation_changed, selection_bounds)?
        } else if self.kind == RepresentationKind::Surface
            && (representation_changed
                || self.synced.is_none_or(|old| {
                    old.quality != current.quality
                        || old.coordinates != current.coordinates
                        || old.overlay_binding != current.overlay_binding
                }))
        {
            let _coordinates_changed = self
                .synced
                .is_none_or(|old| old.coordinates != current.coordinates);
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
        let (records, visibility, acceleration) =
            (input.records, input.visibility, input.acceleration_draw);
        self.bind(&RepresentationBinding {
            device: input.device,
            records,
            visibility,
            acceleration,
            layout: input.layout,
            quality_layout: input.quality_layout,
            structure: input.structure_gpu,
            asset_arena: input.asset_arena,
            surface_field_fallback: input.surface_field_fallback,
            surface_normal_fallback: input.surface_normal_fallback,
            surface_fields: input.surface_fields,
            overlay: input.overlay_view,
            visual_programs: input.visual_program_buffer,
            visual_parameters: input.visual_parameter_buffer,
            visual_properties: input.visual_property_buffer,
        });
    }

    fn bind_cull_input(&mut self, input: &SlotSync<'_, D>) {
        let (records, visibility) = (input.records, input.visibility);
        self.bind_cull(&CullBinding {
            device: input.device,
            records,
            visibility,
            arena: input.draw_arena,
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
        // A spline draws its ribbon geometry, not packed atom instances; the
        // shared record set is consulted only when a visual style samples it.
        let visual_records_changed = input.representation.visual.is_some()
            && self.synced.is_none_or(|old| {
                old.visual_program != current.visual_program || old.record_key != current.record_key
            });
        let _ = selection_bounds;
        let geometry_changed = self.synced.is_none_or(|old| {
            old.ribbon != current.ribbon
                || old.properties != current.properties
                || old.color_overlay != current.color_overlay
                || old.secondary_structure != current.secondary_structure
                || old.record_key != current.record_key
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
                overlay: input.representation.color_overlay.and_then(|overlay| {
                    input
                        .scene
                        .property_for_structure(overlay.classes(), input.structure_gpu.handle)
                        .map(|classes| molgfx_geometry::OverlayColumn::new(overlay, classes))
                }),
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
                || old.visual_property_binding != current.visual_property_binding
                || old.color_overlay != current.color_overlay
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
        // The colour block changes with the scheme, which is presentation
        // state: writing it here is what keeps a scheme change off the record
        // path entirely.
        if let Some(color) = &self.color_uniforms {
            super::color_uniforms::ColorUniforms::new(
                input.representation,
                self.color_column,
                self.overlay_column,
            )
            .write::<D>(input.queue, color);
        }
    }

    /// The shared field this slot shades, when it shades one.
    pub(in crate::scene_gpu) const fn surface_key(
        &self,
    ) -> Option<super::surface_field::SurfaceFieldKey> {
        self.surface.key()
    }
}
