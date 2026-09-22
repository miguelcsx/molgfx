//! Pass two: sync each resident slot against the shared, already-packed records.

use crate::RenderError;
use crate::scene_gpu::acceleration_cache::{AccelerationCache, SharedAcceleration};
use crate::scene_gpu::asset_arena::AssetArena;
use crate::scene_gpu::record_cache::{RecordCache, RecordSetRef};
use crate::scene_gpu::scalar_overlay;
use crate::scene_gpu::slot_types::SlotSync;
use crate::scene_gpu::slots::GpuSlot;
use crate::scene_gpu::structure::GpuStructure;
use crate::scene_gpu::sync::GpuScene;
use crate::scene_gpu::sync::properties;
use crate::scene_gpu::visual_parameters::VisualParameterTable;
use crate::scene_gpu::visual_properties::VisualPropertyTable;
use molgfx_core::{AtomProperty, AtomSelection, Scene};
use molgfx_gpu::Device;

/// Frame inputs every slot in one pass shares.
#[derive(Clone, Copy)]
struct SlotInputs<'a, D: Device> {
    device: &'a D,
    queue: &'a D::Queue,
    scene: &'a Scene,
    quality: bool,
    ray_query_layout: Option<&'a D::BindGroupLayout>,
}

/// The shared read-only tables one pass borrows while syncing every slot.
struct SlotShared<'a, D: Device> {
    /// The scene-wide surface fields the binding pass looks textures up in.
    surface_fields: &'a crate::scene_gpu::surface_cache::SurfaceFieldCache<D>,
    structures: &'a [GpuStructure<D>],
    records: &'a RecordCache<D>,
    args: &'a std::collections::BTreeMap<crate::scene_gpu::RecordKey, (u64, u64, u64)>,
    /// The arena the cull groups bind as ranged slices.
    draw_arena: Option<&'a D::Buffer>,
    acceleration: &'a AccelerationCache<D>,
    visibility: &'a crate::scene_gpu::visibility_cache::VisibilityCache<D>,
    visual_properties: &'a VisualPropertyTable<D>,
    visual_programs: &'a crate::scene_gpu::visual_programs::VisualProgramTable<D>,
    visual_parameters: &'a VisualParameterTable<D>,
    volume_resources: &'a [crate::scene_gpu::volume_slot::GpuVolumeResource<D>],
    surface_field_fallback: &'a D::TextureView,
    surface_normal_fallback: &'a D::TextureView,
    group2_layout: &'a D::BindGroupLayout,
    quality_layout: &'a D::BindGroupLayout,
    ribbon_layout: &'a D::BindGroupLayout,
    atom_cull_layout: &'a D::BindGroupLayout,
    bond_cull_layout: &'a D::BindGroupLayout,
    visual_cull_layout: &'a D::BindGroupLayout,
    frame: &'a D::Buffer,
    cull_tiles: &'a D::Buffer,
    cull_binding_revision: u64,
    asset_arena: &'a AssetArena<D>,
}

impl<D: Device> GpuScene<D> {
    /// Pass two: sync every slot against the now-resident shared records.
    ///
    /// A slot whose key has no resident set — because the budget refused it —
    /// is skipped: it draws nothing this frame rather than drawing stale
    /// records.
    pub(super) fn sync_representation_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
        quality: bool,
        ray_query_layout: Option<&D::BindGroupLayout>,
    ) -> Result<bool, RenderError> {
        let inputs = SlotInputs {
            device,
            queue,
            scene,
            quality,
            ray_query_layout,
        };
        let arena = self.indirect.buffer();
        // Destructured so the slot list and the ribbon scratch can be borrowed
        // mutably while every shared table stays borrowed immutably.
        let shared = SlotShared {
            surface_fields: &self.surface_fields,
            draw_arena: arena,
            structures: &self.structures,
            records: &self.records,
            args: &self.argument_offsets,
            acceleration: &self.acceleration,
            visibility: &self.visibility,
            visual_properties: &self.visual_properties,
            visual_programs: &self.visual_programs,
            visual_parameters: &self.visual_parameters,
            volume_resources: &self.volume_resources,
            surface_field_fallback: &self.surface_field_fallback,
            surface_normal_fallback: &self.surface_normal_fallback,
            group2_layout: &self.group2_layout,
            quality_layout: &self.quality_layout,
            ribbon_layout: &self.ribbon_layout,
            atom_cull_layout: &self.atom_cull_layout,
            bond_cull_layout: &self.bond_cull_layout,
            visual_cull_layout: &self.visual_cull_layout,
            frame: &self.frame_uniforms,
            cull_tiles: &self.cull_tiles,
            cull_binding_revision: self.cull_binding_revision,
            asset_arena: &self.asset_arena,
        };
        let slots = &mut self.slots;
        let ribbon_scratch = &mut self.ribbon_scratch;
        let mut changed = false;
        for (slot_index, slot) in slots.iter_mut().enumerate() {
            changed |= sync_one_slot(slot, slot_index, &inputs, &shared, ribbon_scratch)?;
        }
        Ok(changed)
    }
}

/// Everything one slot resolves from the shared tables.
struct Resolved<'a, D: Device> {
    args: Option<(u64, u64, u64)>,
    records: RecordSetRef<'a, D>,
    record_key: crate::scene_gpu::RecordKey,
    acceleration: Option<&'a SharedAcceleration<D>>,
    selection: &'a AtomSelection,
    selection_handle: molgfx_core::SelectionHandle,
    color_property: Option<&'a AtomProperty>,
    appearance_property: Option<&'a AtomProperty>,
    property_revisions: [u64; 2],
    visual_property_revisions: [u64; 4],
    visual_property_offsets: [u32; 4],
    visual_attribute_layouts: [u32; 4],
    visual_program_offset: u32,
    overlay_volume: Option<&'a molgfx_core::ScalarVolume>,
    overlay_view: &'a D::TextureView,
    overlay_binding_revision: u64,
    representation_revision: u64,
}

/// Resolves one slot's shared inputs, or nothing when its rows are absent.
fn resolve<'a, D: Device>(
    slot: &GpuSlot<D>,
    structure_gpu: &'a GpuStructure<D>,
    scene: &'a Scene,
    shared: &'a SlotShared<'a, D>,
) -> Option<Resolved<'a, D>> {
    let representation = scene.representation(slot.key.representation)?;
    let selection_handle = representation.selection()?;
    let selection = scene.selection_for(selection_handle, slot.key.structure)?;
    let representation_revision = scene.representation_content_revision(slot.key.representation)?;
    let (color_property, appearance_property, property_revisions) =
        properties::resolve(scene, representation, slot.key.structure);
    let (_, visual_property_revisions) =
        properties::resolve_visual(scene, representation, slot.key.structure);
    let visual_attributes =
        shared
            .visual_properties
            .offsets(scene, slot.key.structure, representation.visual.as_ref());
    let visual_program_offset = representation
        .visual
        .as_ref()
        .and_then(|style| shared.visual_programs.offset(style.program()))
        .into_iter()
        .fold(0, |_, offset| offset);
    let (overlay_volume, overlay_view, overlay_binding_revision) = scalar_overlay::resolve(
        shared.volume_resources,
        scene,
        representation,
        shared.surface_field_fallback,
    );
    let record_key = RecordCache::<D>::key(
        scene,
        scene.structure(slot.key.structure)?,
        slot.key.structure,
        representation,
        selection_handle,
        structure_gpu.asset_identity(),
        property_revisions,
    );
    let records = shared.records.get(record_key)?.as_ref()?;
    Some(Resolved {
        args: shared.args.get(&record_key).copied(),
        records,
        record_key,
        acceleration: shared.acceleration.get(record_key),
        selection,
        selection_handle,
        color_property,
        appearance_property,
        property_revisions,
        visual_property_revisions,
        visual_property_offsets: visual_attributes.offsets,
        visual_attribute_layouts: visual_attributes.layouts,
        visual_program_offset,
        overlay_volume,
        overlay_view,
        overlay_binding_revision,
        representation_revision,
    })
}

/// Syncs one resident slot against its shared records.
fn sync_one_slot<D: Device>(
    slot: &mut GpuSlot<D>,
    slot_index: usize,
    inputs: &SlotInputs<'_, D>,
    shared: &SlotShared<'_, D>,
    ribbon_scratch: &mut molgfx_geometry::RibbonMesh,
) -> Result<bool, RenderError> {
    let SlotInputs {
        device,
        queue,
        scene,
        quality,
        ray_query_layout,
    } = *inputs;
    let Some(placed) = scene.structure(slot.key.structure) else {
        return Ok(false);
    };
    let Some(representation) = scene.representation(slot.key.representation) else {
        return Ok(false);
    };
    let Some(structure_gpu) = shared.structures.get(slot.structure_index) else {
        return Ok(false);
    };
    let Some(resolved) = resolve(slot, structure_gpu, scene, shared) else {
        return Ok(false);
    };
    let visibility_key = crate::scene_gpu::VisibilityKey {
        records: resolved.record_key,
        lod_mode: slot.lod_mode(),
        visual_enabled: representation.visual.is_some(),
        bond_break_length: placed.bond_break_length().to_bits(),
    };
    let Some(visibility) = shared
        .visibility
        .get(visibility_key)
        .and_then(super::super::visibility_cache::VisibilitySet::as_ref)
    else {
        return Ok(false);
    };
    slot.visibility_key = Some(visibility_key);
    slot.sync(SlotSync {
        device,
        queue,
        records: resolved.records,
        surface_fields: shared.surface_fields,
        visibility,
        args: resolved.args,
        draw_arena: shared.draw_arena,
        acceleration_draw: resolved.acceleration,
        layout: shared.group2_layout,
        quality_layout: shared.quality_layout,
        ray_query_layout,
        ribbon_layout: shared.ribbon_layout,
        atom_cull_layout: shared.atom_cull_layout,
        bond_cull_layout: shared.bond_cull_layout,
        visual_cull_layout: shared.visual_cull_layout,
        surface_field_fallback: shared.surface_field_fallback,
        surface_normal_fallback: shared.surface_normal_fallback,
        overlay_volume: resolved.overlay_volume,
        overlay_view: resolved.overlay_view,
        overlay_binding_revision: resolved.overlay_binding_revision,
        quality,
        frame: shared.frame,
        cull_tiles: shared.cull_tiles,
        cull_binding_revision: shared.cull_binding_revision,
        structure_gpu,
        asset_arena: shared.asset_arena,
        asset_identity: structure_gpu.asset_identity(),
        scene,
        placed,
        representation,
        representation_revision: resolved.representation_revision,
        selection: resolved.selection,
        selection_handle: resolved.selection_handle,
        color_property: resolved.color_property,
        appearance_property: resolved.appearance_property,
        property_revisions: resolved.property_revisions,
        visual_property_buffer: shared.visual_properties.buffer(),
        visual_program_buffer: shared.visual_programs.buffer(),
        visual_program_offset: resolved.visual_program_offset,
        visual_program_binding_revision: shared.visual_programs.binding_revision(),
        visual_parameter_buffer: shared.visual_parameters.buffer(),
        visual_parameter_offset: VisualParameterTable::<D>::offset(slot_index),
        visual_parameter_binding_revision: shared.visual_parameters.binding_revision(),
        visual_property_offsets: resolved.visual_property_offsets,
        visual_attribute_layouts: resolved.visual_attribute_layouts,
        visual_state_offset: shared.visual_properties.state_offset(slot.key.structure),
        visual_property_binding_revision: shared.visual_properties.binding_revision(),
        visual_property_revisions: resolved.visual_property_revisions,
        visual_time_seconds: scene.presentation_time_seconds(),
        visual_time_revision: scene.presentation_revision(),
        ribbon: ribbon_scratch,
    })
}
