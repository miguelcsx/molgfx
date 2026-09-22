//! Revision-diffed synchronization of persistent scene GPU state.
//!
//! Reconciliation is linear in resident slots; unchanged frames do no uploads.
mod assets;
mod draws;
mod fallback;
mod init;
mod paged_instances;
pub(crate) use paged_instances::PagedInstancesSync;
pub(crate) use paged_relations::PagedRelationsSync;
mod paged_relations;
mod properties;
mod relations;
mod residency_init;
mod scene_identity;
mod segmentations;
pub(super) mod selection_bounds;
mod semantic_tables;
mod trajectories;
mod upload_scratch;
mod volumes;
use super::asset::GpuAsset;
use super::asset_arena::AssetArena;
use super::buffers::create_cull_tiles;
use super::instance_batch_table::GpuInstanceBatches;
use super::interaction_table::GpuInteractions;
use super::label_table::GpuLabels;
use super::ligand_pose_table::GpuLigandPoses;
use super::overlay_table::GpuOverlays;
use super::paged_bonds::PagedBondBatch;
use super::paged_chunks::PagedChunkBatch;
use super::picking_pages::PickPages;
use super::point_batch_table::GpuPointBatches;
use super::primitive_table::GpuPrimitives;
use super::segmentation_slot::{GpuSegmentationResource, GpuSegmentationSlot};
use super::slot_types::{SlotKey, SlotPlan, SlotSync};
use super::slots::GpuSlot;
use super::structure::GpuStructure;
use super::uniforms::FrameUniforms;
use super::visual::VisualFallback;
use super::visual_parameters::VisualParameterTable;
use super::visual_programs::VisualProgramTable;
use super::visual_properties::VisualPropertyTable;
use super::volume_slot::{GpuVolumeResource, GpuVolumeSlot};
use crate::error::RenderError;
use crate::{ResidencyMachine, ResidencyTicket, ResidencyWorkspace};
use molgfx_core::{
    AtomGpu, BondGpu, RepresentationHandle, Scene, SegmentationHandle, VolumeHandle,
};
use molgfx_gpu::{ArenaAllocation, Device, UploadTicket};
use semantic_tables::SemanticSync;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
struct FrameUploadCommand {
    ticket: UploadTicket,
    offset: usize,
    len: usize,
}

pub(crate) struct SceneSync<'a, D: Device> {
    pub(crate) device: &'a D,
    pub(crate) queue: &'a D::Queue,
    pub(crate) scene: &'a Scene,
    pub(crate) quality: bool,
    pub(crate) extent: [u32; 2],
    pub(crate) ray_query_layout: Option<&'a D::BindGroupLayout>,
    pub(crate) derived_cache: &'a mut crate::DerivedCache,
    pub(crate) derived_frame: u64,
}

/// GPU-resident scene state with stable structure and representation slots.
#[derive(Debug)]
pub(crate) struct GpuScene<D: Device> {
    pub frame_uniforms: D::Buffer,
    pub group0: D::BindGroup,
    pub group0_layout: D::BindGroupLayout,
    pub group2_layout: D::BindGroupLayout,
    pub quality_layout: D::BindGroupLayout,
    pub volume_layout: D::BindGroupLayout,
    pub segmentation_layout: D::BindGroupLayout,
    pub surface_field_output_layout: D::BindGroupLayout,
    pub surface_field_input_layout: D::BindGroupLayout,
    pub surface_field_erosion_layout: D::BindGroupLayout,
    pub surface_field_normal_layout: D::BindGroupLayout,
    pub surface_component_layout: D::BindGroupLayout,
    pub ribbon_layout: D::BindGroupLayout,
    pub atom_cull_layout: D::BindGroupLayout,
    pub bond_cull_layout: D::BindGroupLayout,
    pub visual_cull_layout: D::BindGroupLayout,
    cull_tiles: D::Buffer,
    cull_tiles_capacity: u64,
    cull_binding_revision: u64,
    cull_tile_count: u32,
    pub interaction_layout: D::BindGroupLayout,
    pub relation_cull_layout: D::BindGroupLayout,
    pub relation_resolve_layout: D::BindGroupLayout,
    pub generic_point_cull_layout: D::BindGroupLayout,
    pub generic_point_render_layout: D::BindGroupLayout,
    pub generic_instance_cull_layout: D::BindGroupLayout,
    pub generic_instance_render_layout: D::BindGroupLayout,
    pub instance_timeline_layout: D::BindGroupLayout,
    pub attribute_timeline_layout: D::BindGroupLayout,
    pub primitive_layout: D::BindGroupLayout,
    pub primitive_motion_layout: D::BindGroupLayout,
    pub primitive_shadow_layout: D::BindGroupLayout,
    pub ligand_pose_layout: D::BindGroupLayout,
    pub label_declutter_layout: D::BindGroupLayout,
    pub label_render_layout: D::BindGroupLayout,
    pub overlay_layout: D::BindGroupLayout,
    pub trajectory_layout: D::BindGroupLayout,
    pub occupancy_layout: D::BindGroupLayout,
    _surface_field_fallback_texture: D::Texture,
    surface_field_fallback: D::TextureView,
    _surface_normal_fallback_texture: D::Texture,
    surface_normal_fallback: D::TextureView,
    asset_arena: AssetArena<D>,
    assets: Vec<Arc<GpuAsset<D>>>,
    structures: Vec<GpuStructure<D>>,
    slots: Vec<GpuSlot<D>>,
    volume_resources: Vec<GpuVolumeResource<D>>,
    pub(super) brick_atlases: Vec<super::brick_atlas::upload::GpuBrickAtlas<D>>,
    volume_slots: Vec<GpuVolumeSlot<D>>,
    mesh_slots: Vec<super::mesh_slot::GpuMeshSlot<D>>,
    mesh_synced: Option<(u64, u64)>,
    segmentation_resources: Vec<GpuSegmentationResource<D>>,
    segmentation_slots: Vec<GpuSegmentationSlot<D>>,
    interactions: GpuInteractions<D>,
    point_batches: GpuPointBatches<D>,
    instance_batches: GpuInstanceBatches<D>,
    primitive: GpuPrimitives<D>,
    ligand_poses: GpuLigandPoses<D>,
    labels: GpuLabels<D>,
    overlays: GpuOverlays<D>,
    pub(super) paged_chunks: PagedChunkBatch<D>,
    pub(super) paged_bonds: PagedBondBatch<D>,
    pub(super) picking_pages: PickPages,
    paged_instance_pick_scratch: Vec<super::picking_pages::ChunkPickPlan>,
    paged_relation_pick_scratch: Vec<super::picking_pages::ChunkPickPlan>,
    visual_properties: VisualPropertyTable<D>,
    visual_programs: VisualProgramTable<D>,
    visual_parameters: VisualParameterTable<D>,
    visual_fallback: VisualFallback<D>,
    paged_visual_time_seconds: f32,
    paged_visual_time_revision: u64,
    scene_identity: Option<u64>,
    structure_revision: Option<u64>,
    slot_structure_revision: Option<u64>,
    representation_revision: Option<u64>,
    volume_slot_revision: Option<(u64, u64)>,
    segmentation_slot_revision: Option<(u64, u64)>,
    representation_scratch: Vec<(u16, RepresentationHandle)>,
    volume_handle_scratch: Vec<VolumeHandle>,
    segmentation_handle_scratch: Vec<SegmentationHandle>,
    plan_scratch: Vec<SlotPlan>,
    atom_scratch: Vec<AtomGpu>,
    bond_scratch: Vec<BondGpu>,
    compaction_scratch: Vec<u32>,
    ribbon_scratch: molgfx_geometry::RibbonMesh,
    residency: ResidencyWorkspace<FrameUploadCommand>,
    residency_machine: ResidencyMachine,
    frame_residency_ticket: ResidencyTicket,
    _frame_allocation: ArenaAllocation,
    upload_fence: u64,
}

impl<D: Device> GpuScene<D> {
    /// Rebuilds resident caller meshes when the mesh table or the structures
    /// they hang from change. Untouched meshes keep their buffers.
    fn sync_mesh_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let revision = (scene.mesh_revision(), scene.structure_revision());
        if self.mesh_synced == Some(revision) {
            return Ok(false);
        }
        self.mesh_slots.clear();
        for (mesh_handle, mesh) in scene.meshes() {
            if !mesh.visible() {
                continue;
            }
            let Some(structure) = self
                .structures
                .iter()
                .find(|structure| structure.handle == mesh.owner())
            else {
                continue;
            };
            let mut slot = super::mesh_slot::GpuMeshSlot::new();
            let instances: Vec<_> = scene.mesh_instance_transforms(mesh_handle).collect();
            slot.sync(
                device,
                queue,
                mesh,
                (
                    &self.ribbon_layout,
                    structure,
                    self.visual_fallback.entries(),
                ),
                super::mesh_slot::MeshOccurrences {
                    transforms: &instances,
                    model_to_world: scene
                        .structure(mesh.owner())
                        .map_or(molgfx_math::Mat4::IDENTITY, |placed| placed.model_to_world),
                    entity: molgfx_core::EntityId::pack(
                        molgfx_core::EntityKind::Mesh,
                        u64::from(molgfx_core::Scene::mesh_row(mesh_handle)),
                    )?,
                },
            )?;
            self.mesh_slots.push(slot);
        }
        self.mesh_synced = Some(revision);
        Ok(true)
    }

    pub(crate) fn sync(&mut self, input: SceneSync<'_, D>) -> Result<bool, RenderError> {
        let SceneSync {
            device,
            queue,
            scene,
            quality,
            extent,
            ray_query_layout,
            derived_cache,
            derived_frame,
        } = input;
        self.ensure_cull_tiles(device, extent)?;
        self.paged_visual_time_seconds = scene.presentation_time_seconds();
        self.paged_visual_time_revision = scene.presentation_revision();
        let scene_changed = self.begin_scene(scene.cache_identity())?;
        let mut changed = scene_changed
            || self.structure_revision != Some(scene.structure_revision())
            || self.representation_revision != Some(scene.representation_revision())
            || self.volume_slot_revision
                != Some((scene.volume_revision(), scene.representation_revision()))
            || self.segmentation_slot_revision
                != Some((
                    scene.segmentation_revision(),
                    scene.representation_revision(),
                ));
        for atlas in &mut self.brick_atlases {
            let retired = atlas.poll(device, queue)?;
            changed |= retired.uploads_published != 0 || retired.evictions_completed != 0;
        }
        let requires_bvh = quality
            || scene.representations().any(|(_, representation)| {
                representation.kind == molgfx_core::RepresentationKind::Surface
            });
        changed |= self.reconcile_structures(device, queue, scene)?;
        changed |= self.picking_pages.sync(scene)?;
        for structure in &mut self.structures {
            let Some(placed) = scene.structure(structure.handle) else {
                return Err(RenderError::PickingOwnerMissing);
            };
            changed |= structure.set_pick_pages(self.picking_pages.pages_for(placed.dataset_id()));
        }
        self.reconcile_slots(scene);
        changed |= self.visual_programs.sync(device, queue, scene)?;
        changed |= self.visual_parameters.reserve(
            device,
            self.slots
                .len()
                .saturating_add(scene.domain_visuals().count()),
        )?;
        changed |= self.visual_properties.sync(
            device,
            queue,
            scene,
            &self.attribute_timeline_layout,
            derived_cache,
            derived_frame,
        )?;
        self.reconcile_volume_slots(scene);
        self.reconcile_segmentations(scene);
        changed |= self.sync_segmentation_resources(device, queue, scene)?;
        let mut dynamic_sources_changed = false;
        for gpu in &mut self.structures {
            if let Some(placed) = scene.structure(gpu.handle) {
                let source_changed =
                    gpu.sync(device, queue, placed, &self.trajectory_layout, requires_bvh)?;
                dynamic_sources_changed |= source_changed;
                changed |= source_changed;
            }
        }
        changed |= self.sync_semantic_tables(SemanticSync {
            device,
            queue,
            scene,
            quality,
            extent,
            dynamic_sources_changed,
            derived_cache,
            derived_frame,
        })?;
        changed |= self.sync_volume_resources(device, queue, scene)?;
        changed |=
            self.sync_representation_slots(device, queue, scene, quality, ray_query_layout)?;
        self.release_upload_scratch();
        changed |= self.sync_volume_slots(device, queue, scene)?;
        changed |= self.sync_mesh_slots(device, queue, scene)?;
        changed |= self.sync_segmentation_slots(device, queue, scene)?;
        Ok(changed)
    }

    fn sync_representation_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
        quality: bool,
        ray_query_layout: Option<&D::BindGroupLayout>,
    ) -> Result<bool, RenderError> {
        let mut changed = false;
        for (slot_index, slot) in self.slots.iter_mut().enumerate() {
            let Some(placed) = scene.structure(slot.key.structure) else {
                continue;
            };
            let Some(representation) = scene.representation(slot.key.representation) else {
                continue;
            };
            let Some(selection_handle) = representation.selection() else {
                continue;
            };
            let Some(selection) = scene.selection_for(selection_handle, slot.key.structure) else {
                continue;
            };
            let Some(representation_revision) =
                scene.representation_content_revision(slot.key.representation)
            else {
                continue;
            };
            let Some(structure_gpu) = self.structures.get(slot.structure_index) else {
                continue;
            };
            let (color_property, appearance_property, property_revisions) =
                properties::resolve(scene, representation, slot.key.structure);
            let (_, visual_property_revisions) =
                properties::resolve_visual(scene, representation, slot.key.structure);
            let visual_attributes = self.visual_properties.offsets(
                scene,
                slot.key.structure,
                representation.visual.as_ref(),
            );
            let visual_program_offset = representation
                .visual
                .as_ref()
                .and_then(|style| self.visual_programs.offset(style.program()))
                .into_iter()
                .fold(0, |_, offset| offset);
            let (overlay_volume, overlay_view, overlay_binding_revision) =
                super::scalar_overlay::resolve(
                    &self.volume_resources,
                    scene,
                    representation,
                    &self.surface_field_fallback,
                );
            changed |= slot.sync(SlotSync {
                device,
                queue,
                layout: &self.group2_layout,
                quality_layout: &self.quality_layout,
                ray_query_layout,
                ribbon_layout: &self.ribbon_layout,
                atom_cull_layout: &self.atom_cull_layout,
                bond_cull_layout: &self.bond_cull_layout,
                visual_cull_layout: &self.visual_cull_layout,
                surface_field_output_layout: &self.surface_field_output_layout,
                surface_field_input_layout: &self.surface_field_input_layout,
                surface_field_erosion_layout: &self.surface_field_erosion_layout,
                surface_field_normal_layout: &self.surface_field_normal_layout,
                surface_component_layout: &self.surface_component_layout,
                surface_field_fallback: &self.surface_field_fallback,
                surface_normal_fallback: &self.surface_normal_fallback,
                overlay_volume,
                overlay_view,
                overlay_binding_revision,
                quality,
                frame: &self.frame_uniforms,
                cull_tiles: &self.cull_tiles,
                cull_binding_revision: self.cull_binding_revision,
                structure_gpu,
                asset_arena: &self.asset_arena,
                placed,
                representation,
                representation_revision,
                selection,
                color_property,
                appearance_property,
                property_revisions,
                visual_property_buffer: self.visual_properties.buffer(),
                visual_program_buffer: self.visual_programs.buffer(),
                visual_program_offset,
                visual_program_binding_revision: self.visual_programs.binding_revision(),
                visual_parameter_buffer: self.visual_parameters.buffer(),
                visual_parameter_offset: VisualParameterTable::<D>::offset(slot_index),
                visual_parameter_binding_revision: self.visual_parameters.binding_revision(),
                visual_property_offsets: visual_attributes.offsets,
                visual_attribute_layouts: visual_attributes.layouts,
                visual_state_offset: self.visual_properties.state_offset(slot.key.structure),
                visual_property_binding_revision: self.visual_properties.binding_revision(),
                visual_property_revisions,
                visual_time_seconds: scene.presentation_time_seconds(),
                visual_time_revision: scene.presentation_revision(),
                atoms: &mut self.atom_scratch,
                bonds: &mut self.bond_scratch,
                compaction: &mut self.compaction_scratch,
                ribbon: &mut self.ribbon_scratch,
            })?;
        }
        Ok(changed)
    }

    fn ensure_cull_tiles(&mut self, device: &D, extent: [u32; 2]) -> Result<(), RenderError> {
        self.cull_tile_count = extent[0].div_ceil(8).saturating_mul(extent[1].div_ceil(8));
        let bytes = u64::from(self.cull_tile_count)
            .saturating_mul(std::mem::size_of::<u32>() as u64)
            .max(256);
        if bytes > self.cull_tiles_capacity {
            (self.cull_tiles, self.cull_tiles_capacity) = create_cull_tiles(device, bytes)?;
            self.cull_binding_revision = self.cull_binding_revision.wrapping_add(1);
        }
        Ok(())
    }

    fn reconcile_slots(&mut self, scene: &Scene) {
        let revision = scene.representation_revision();
        if self.representation_revision == Some(revision)
            && self.slot_structure_revision == Some(scene.structure_revision())
        {
            return;
        }
        self.representation_scratch.clear();
        self.representation_scratch.extend(
            scene
                .representations()
                .filter(|(_, rep)| rep.visible)
                .filter(|(_, rep)| rep.selection().is_some())
                .map(|(handle, rep)| (rep.order, handle)),
        );
        self.representation_scratch.sort_unstable();
        self.plan_scratch.clear();
        for (_, representation) in &self.representation_scratch {
            for (structure_index, structure) in self.structures.iter().enumerate() {
                let Some(value) = scene.representation(*representation) else {
                    continue;
                };
                let Some(selection) = value.selection() else {
                    continue;
                };
                let Some(selection) = scene.selection_for(selection, structure.handle) else {
                    continue;
                };
                let Some(placed) = scene.structure(structure.handle) else {
                    continue;
                };
                if selection.count(placed.atoms.len()) == 0 {
                    continue;
                }
                self.plan_scratch.push(SlotPlan {
                    key: SlotKey {
                        structure: structure.handle,
                        representation: *representation,
                    },
                    structure_index,
                });
            }
        }
        let mut old = std::mem::take(&mut self.slots);
        for plan in &self.plan_scratch {
            if let Some(index) = old.iter().position(|slot| slot.key == plan.key) {
                let mut slot = old.swap_remove(index);
                slot.structure_index = plan.structure_index;
                self.slots.push(slot);
            } else {
                self.slots.push(GpuSlot::new(*plan));
            }
        }
        self.representation_revision = Some(revision);
        self.slot_structure_revision = Some(scene.structure_revision());
    }
}

include!("sync/volume_reconcile.rs");
