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
mod helix_fits;
mod paged_relations;
mod properties;
mod relations;
mod residency_init;
mod scene_identity;
mod segmentations;
pub(super) mod selection_bounds;
mod semantic_tables;
mod shared_caches;
mod slot_reconcile;
mod slot_sync;
mod specialize;
mod surface_resolve;
mod trajectories;
mod upload_scratch;
mod volume_boundaries;
mod volume_reconcile;
mod volumes;
use super::asset::GpuAsset;
use super::asset_arena::AssetArena;
use super::buffers::{create_cull_tiles, cull_tiles_bytes};
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
use super::slot_types::{LOD_ATOM_THRESHOLD, SlotPlan};
use super::slots::GpuSlot;
use super::structure::GpuStructure;
use super::uniforms::FrameUniforms;
use super::visual::VisualFallback;
use super::visual_parameters::VisualParameterTable;
use super::visual_programs::VisualProgramTable;
use super::visual_properties::VisualPropertyTable;
use super::volume_slot::{GpuVolumeResource, GpuVolumeSlot};
use crate::error::RenderError;
use crate::{ResidencyMachine, ResidencyWorkspace};
use molgfx_core::{
    AtomGpu, BondGpu, RepresentationHandle, Scene, SegmentationHandle, VolumeHandle,
};
use molgfx_gpu::{Device, TextureFormat, UploadTicket};
use semantic_tables::SemanticSync;
use std::collections::BTreeMap;
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
    /// Sampling density the quality tier allows.
    pub(crate) detail: super::detail::TierDetail,
    pub(crate) extent: [u32; 2],
    pub(crate) ray_query_layout: Option<&'a D::BindGroupLayout>,
    pub(crate) derived_cache: &'a mut crate::DerivedCache,
    pub(crate) derived_frame: u64,
}

/// Whether the occlusion cull computed for the current view can stand in for
/// the next samples of the same view. Atomics keep it readable through `&self`
/// from the record passes.
#[derive(Debug, Default)]
pub(crate) struct CullViewState {
    /// A full cull was recorded and nothing it read has changed since.
    valid: std::sync::atomic::AtomicBool,
    /// The frame being recorded only moved the sub-pixel jitter.
    reuse: std::sync::atomic::AtomicBool,
    /// The realtime frame being prepared kept its camera and scene.
    staged: std::sync::atomic::AtomicBool,
    /// Scratch the record passes retain between frames.
    scratch: super::cull_scratch::DrawScratch,
}

/// The presentation clock the paged visual programs last read.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PagedVisualTime {
    seconds: f32,
    revision: u64,
}

/// GPU-resident scene state with stable structure and representation slots.
#[derive(Debug)]
pub(crate) struct GpuScene<D: Device> {
    pub frame_uniforms: D::Buffer,
    pub group0: D::BindGroup,
    pub group0_layout: D::BindGroupLayout,
    pub(super) exposure_uniforms: Option<super::exposure_arena::ExposureArena<D>>,
    pub group2_layout: D::BindGroupLayout,
    pub quality_layout: D::BindGroupLayout,
    pub volume_layout: D::BindGroupLayout,
    pub segmentation_layout: D::BindGroupLayout,
    pub field_boundary_layout: D::BindGroupLayout,
    pub surface_field_output_layout: D::BindGroupLayout,
    pub surface_field_input_layout: D::BindGroupLayout,
    pub surface_field_erosion_layout: D::BindGroupLayout,
    pub surface_component_layout: D::BindGroupLayout,
    pub ribbon_layouts: super::layouts::RibbonLayouts<D>,
    pub atom_cull_layout: D::BindGroupLayout,
    pub bond_cull_layout: D::BindGroupLayout,
    pub visual_cull_layout: D::BindGroupLayout,
    cull_tiles: D::Buffer,
    cull_tiles_capacity: u64,
    cull_binding_revision: u64,
    cull_tile_count: u32,
    /// Whether the occlusion cull of the last recorded view still stands, and
    /// whether the frame being recorded may reuse it (see `view_reuse`).
    cull_view: CullViewState,
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
    pub occupancy: Option<(D::BindGroupLayout, TextureFormat)>,
    /// The scene-wide implicit-surface field cache. Fields are shared by
    /// geometry and sampling policy, so two surfaces differing only in
    /// appearance generate one field.
    surface_fields: crate::scene_gpu::surface_cache::SurfaceFieldCache<D>,
    _surface_field_fallback_texture: D::Texture,
    surface_field_fallback: D::TextureView,
    asset_arena: AssetArena<D>,
    assets: Vec<Arc<GpuAsset<D>>>,
    structures: Vec<GpuStructure<D>>,
    slots: Vec<GpuSlot<D>>,
    volume_resources: Vec<GpuVolumeResource<D>>,
    volume_boundaries: Vec<super::volume_boundary::VolumeBoundary<D>>,
    volume_boundary_revision: Option<(u64, u64)>,
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
    records: super::record_cache::RecordCache<D>,
    visibility: super::visibility_cache::VisibilityCache<D>,
    acceleration: super::acceleration_cache::AccelerationCache<D>,
    indirect: super::indirect_arena::IndirectArgsArena<D>,
    /// Argument-slot offsets per record key, resolved in the record pass.
    argument_offsets: BTreeMap<(super::VisibilityKey, u32), (u64, u64, u64)>,
    visual_properties: VisualPropertyTable<D>,
    visual_programs: VisualProgramTable<D>,
    visual_parameters: VisualParameterTable<D>,
    visual_fallback: VisualFallback<D>,
    /// Generated pipelines, one per style, stage and drawable family.
    ///
    /// Settled once per frame before any pass records, then read through a
    /// shared borrow while draws are recorded.
    pub(super) specialized: crate::engine::pipeline_cache::SpecializedPipelines<D>,
    paged_visual_time: PagedVisualTime,
    scene_identity: Option<u64>,
    structure_revision: Option<u64>,
    slot_structure_revision: Option<u64>,
    representation_revision: Option<u64>,
    representation_membership_revision: Option<u64>,
    volume_slot_revision: Option<(u64, u64)>,
    segmentation_slot_revision: Option<(u64, u64)>,
    representation_scratch: Vec<(u16, RepresentationHandle)>,
    volume_handle_scratch: Vec<VolumeHandle>,
    segmentation_handle_scratch: Vec<SegmentationHandle>,
    plan_scratch: Vec<SlotPlan>,
    atom_scratch: Vec<AtomGpu>,
    bond_scratch: Vec<BondGpu>,
    compaction_scratch: Vec<u32>,
    /// Reused per-frame inputs for the record, visibility and argument syncs.
    /// Kept together and retained across frames so a steady-state frame rebuilds
    /// these containers into existing capacity instead of allocating.
    sync_scratch: SyncScratch,
    ribbon_scratch: molgfx_geometry::RibbonMesh,
    residency: ResidencyWorkspace<FrameUploadCommand>,
    residency_machine: ResidencyMachine,
    frame_residency: residency_init::FrameUniformResidency,
}

/// Reusable per-frame containers for the three sync passes.
///
/// Every field is cleared and refilled each frame rather than reallocated, so
/// a steady-state frame's bookkeeping allocates nothing.
#[derive(Debug, Default)]
pub(crate) struct SyncScratch {
    /// Distinct record keys this frame needs, mapped to a representative slot.
    pub(super) needed_records:
        std::collections::BTreeMap<crate::scene_gpu::record_cache::RecordKey, usize>,
    /// One culling policy per slot.
    pub(super) cull_policies: Vec<crate::scene_gpu::slot_types::CullPolicy>,
    /// Distinct visibility keys and their packed counts.
    pub(super) visibility_needed: std::collections::BTreeMap<
        crate::scene_gpu::VisibilityKey,
        (u32, u32, crate::scene_gpu::buffers::CullCountInput),
    >,
    /// Live indirect-argument slots, rebuilt per frame and passed to `retain`.
    pub(super) live_argument_slots: Vec<crate::scene_gpu::IndirectSlotKey>,
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
                    &self.ribbon_layouts.draw,
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
            detail,
            extent,
            ray_query_layout,
            derived_cache,
            derived_frame,
        } = input;
        self.ensure_cull_tiles(device, extent, scene.atom_count())?;
        self.paged_visual_time.seconds = scene.presentation_time_seconds();
        self.paged_visual_time.revision = scene.presentation_revision();
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
        let requires_bvh = requires_spatial_hierarchy(scene, quality);
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
        changed |= self.sync_volume_boundaries(device, queue, scene)?;
        self.sync_records(
            device,
            queue,
            scene,
            (requires_bvh, detail),
            derived_cache,
            derived_frame,
        )?;
        changed |= self.sync_molecular_slots(
            device,
            queue,
            scene,
            (quality, detail),
            ray_query_layout,
            (derived_cache, derived_frame),
        )?;
        changed |= self.sync_auxiliary_slots(device, queue, scene)?;
        self.representation_revision = Some(scene.representation_revision());
        Ok(changed)
    }

    /// Surface fields first, then every record-backed representation slot that
    /// binds them, then the release of fields no slot kept.
    fn sync_molecular_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
        (quality, detail): (bool, super::detail::TierDetail),
        ray_query_layout: Option<&D::BindGroupLayout>,
        (derived_cache, derived_frame): (&mut crate::DerivedCache, u64),
    ) -> Result<bool, RenderError> {
        self.prepare_surface_fields(device, queue, scene, detail)?;
        let changed = self.sync_representation_slots(
            device,
            queue,
            scene,
            quality,
            detail,
            ray_query_layout,
        )?;
        self.retain_surface_fields(derived_cache, derived_frame);
        self.release_upload_scratch();
        Ok(changed)
    }

    /// Volume, mesh and segmentation slots: the representations that own no
    /// molecular records and so sync after every record-backed slot.
    fn sync_auxiliary_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = self.sync_volume_slots(device, queue, scene)?;
        changed |= self.sync_mesh_slots(device, queue, scene)?;
        changed |= self.sync_segmentation_slots(device, queue, scene)?;
        Ok(changed)
    }

    /// The scene-wide indirect-argument arena every draw reads from.
    ///
    /// A pass gets this from its context rather than from per-draw arguments,
    /// because the arena is scene state, not draw state.
    pub(crate) fn indirect_args(&self) -> Option<&D::Buffer> {
        self.indirect.buffer()
    }

    fn ensure_cull_tiles(
        &mut self,
        device: &D,
        extent: [u32; 2],
        atoms: u64,
    ) -> Result<(), RenderError> {
        let occlusion = atoms >= u64::from(LOD_ATOM_THRESHOLD);
        self.cull_tile_count = extent[0].div_ceil(8).saturating_mul(extent[1].div_ceil(8));
        let bytes = cull_tiles_bytes(extent, self.cull_tile_count, occlusion);
        if bytes > self.cull_tiles_capacity {
            (self.cull_tiles, self.cull_tiles_capacity) = create_cull_tiles(device, bytes)?;
            self.cull_binding_revision = self.cull_binding_revision.wrapping_add(1);
        }
        Ok(())
    }
}

fn requires_spatial_hierarchy(scene: &molgfx_core::Scene, quality: bool) -> bool {
    quality
        || scene.representations().any(|(_, representation)| {
            matches!(
                representation.kind,
                molgfx_core::RepresentationKind::Surface | molgfx_core::RepresentationKind::Solvent
            )
        })
}
