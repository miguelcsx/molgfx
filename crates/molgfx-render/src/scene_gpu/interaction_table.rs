//! One revision-diffed glyph table plus branch-free dynamic relation streams.

#[path = "relations/paged.rs"]
mod paged;
#[path = "relations/visual.rs"]
mod paged_visual;
pub(in crate::scene_gpu) use paged::PagedRelationSync;

use super::asset_arena::AssetArena;
use super::buffers::{count, write_draw_args};
use super::dispatch::workgroups_2d;
use super::generic_visual::{GenericVisualResources, GenericVisualState, GenericVisualTarget};
use super::grow_buffer::GrowBuffer;
use super::instance_batch_table::GpuInstanceBatches;
use super::picking_pages::PickPages;
use super::point_batch_table::GpuPointBatches;
use super::relation_anchor::{StreamKey, anchor_payload, pipeline_index, stream_key};
use super::structure::GpuStructure;
use super::uniforms::ModelUniforms;
use super::visual::VisualCullEntries;
use crate::error::RenderError;
use molgfx_core::{
    ChunkId, DatasetId, EntityKind, InteractionGpu, RelationBatchHandle, RowDomain, Scene,
};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue as _};
use molgfx_math::Mat4;
use paged_visual::{PagedRelationVisualArenas, PagedRelationVisualState, PagedVisualSync};
use std::collections::BTreeMap;

const RESOLVER_ALIGNMENT_ROWS: usize = 4;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct RelationResolverGpu {
    start: [f32; 4],
    end: [f32; 4],
    output: [u32; 4],
    reserved: [u32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct RelationCullConfig {
    counts: [u32; 4],
}

#[derive(Clone, Copy, Debug)]
struct StreamPlan {
    key: StreamKey,
    first: u32,
    count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelationVisualSource {
    Fallback,
    Scene(RelationBatchHandle),
    Paged(molgfx_core::ChunkOccurrenceId),
}

#[derive(Clone, Copy, Debug)]
struct RelationVisualPlan {
    source: RelationVisualSource,
    first: u32,
    count: u32,
    color: molgfx_math::Rgba8,
    opacity: f32,
}

#[derive(Debug)]
struct RelationStream<D: Device> {
    pipeline: u8,
    tracks_coordinates: bool,
    groups: [u32; 2],
    group: D::BindGroup,
    _start_model: Option<D::Buffer>,
    _end_model: Option<D::Buffer>,
    start_timeline: Option<PagedRigidTimeline>,
    end_timeline: Option<PagedRigidTimeline>,
    start_config: Option<D::Buffer>,
    end_config: Option<D::Buffer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PagedRigidTimeline {
    start_byte_offset: u64,
    end_byte_offset: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct RelationResolveDispatch<'a, D: Device> {
    pub(crate) pipeline: usize,
    pub(crate) groups: [u32; 2],
    pub(crate) group: &'a D::BindGroup,
}

#[derive(Clone, Copy)]
pub(crate) struct RelationCullDispatch<'a, D: Device> {
    pub(crate) groups: [u32; 2],
    pub(crate) group: &'a D::BindGroup,
}

#[derive(Debug)]
struct RelationCullStream<D: Device> {
    source: RelationVisualSource,
    groups: [u32; 2],
    _config: D::Buffer,
    group: D::BindGroup,
    bindings: [u64; 4],
}

#[derive(Debug)]
pub(super) struct GpuInteractions<D: Device> {
    base: GrowBuffer<D>,
    buffer: GrowBuffer<D>,
    visible: GrowBuffer<D>,
    resolvers: GrowBuffer<D>,
    args: Option<D::Buffer>,
    render_group: Option<D::BindGroup>,
    source_fallback: Option<D::Buffer>,
    model_fallback: Option<D::Buffer>,
    timeline_fallback: Option<D::Buffer>,
    streams: Vec<RelationStream<D>>,
    cull_streams: Vec<RelationCullStream<D>>,
    visuals: BTreeMap<RelationBatchHandle, GenericVisualState<D>>,
    paged_visuals: BTreeMap<molgfx_core::ChunkOccurrenceId, PagedRelationVisualState<D>>,
    paged_visual_arenas: PagedRelationVisualArenas<D>,
    visual_plans: Vec<RelationVisualPlan>,
    synced: Option<(u64, u64, u64, u64, u64, u64)>,
    visual_synced: Option<(u64, u64, u64, u64, u64, u64)>,
    paged_synced: Option<(u64, u64, u64, u64)>,
    paged_visual_synced: Option<(u64, u64, u64)>,
    paged_timeline_synced: u64,
    dynamic_state_revision: Option<(u64, u64)>,
    scene_count: usize,
    scene_visual_count: usize,
    scene_resolver_count: usize,
    scene_stream_count: usize,
    count: u32,
    dynamic_dirty: bool,
    scratch: Vec<InteractionGpu>,
    resolver_scratch: Vec<RelationResolverGpu>,
}

pub(super) struct RelationSources<'a, D: Device> {
    pub(super) structures: &'a [GpuStructure<D>],
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) points: &'a GpuPointBatches<D>,
    pub(super) instances: &'a GpuInstanceBatches<D>,
}

#[derive(Clone, Copy)]
pub(super) struct RelationSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) render_layout: &'a D::BindGroupLayout,
    pub(super) cull_layout: &'a D::BindGroupLayout,
    pub(super) resolve_layout: &'a D::BindGroupLayout,
    pub(super) scene: &'a Scene,
    pub(super) sources: RelationSources<'a, D>,
    pub(super) picking: &'a PickPages,
    pub(super) dynamic_sources_changed: bool,
    pub(super) visual: GenericVisualResources<'a, D>,
}

impl<D: Device> Copy for RelationSources<'_, D> {}

impl<D: Device> Clone for RelationSources<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: Device> GpuInteractions<D> {
    pub(super) const fn new() -> Self {
        Self {
            base: GrowBuffer::new(),
            buffer: GrowBuffer::new(),
            visible: GrowBuffer::new(),
            resolvers: GrowBuffer::new(),
            args: None,
            render_group: None,
            source_fallback: None,
            model_fallback: None,
            timeline_fallback: None,
            streams: Vec::new(),
            cull_streams: Vec::new(),
            visuals: BTreeMap::new(),
            paged_visuals: BTreeMap::new(),
            paged_visual_arenas: PagedRelationVisualArenas::new(),
            visual_plans: Vec::new(),
            synced: None,
            visual_synced: None,
            paged_synced: None,
            paged_visual_synced: None,
            paged_timeline_synced: 0,
            dynamic_state_revision: None,
            scene_count: 0,
            scene_visual_count: 0,
            scene_resolver_count: 0,
            scene_stream_count: 0,
            count: 0,
            dynamic_dirty: false,
            scratch: Vec::new(),
            resolver_scratch: Vec::new(),
        }
    }

    pub(super) fn sync(&mut self, input: &RelationSync<'_, D>) -> Result<bool, RenderError> {
        let RelationSync {
            device,
            queue,
            cull_layout,
            scene,
            sources,
            picking,
            dynamic_sources_changed,
            visual,
            ..
        } = *input;
        let revision = (
            scene.interaction_revision(),
            scene.guide_revision(),
            scene.generic_batch_revision(),
            picking.scene_table_revision(),
            sources.asset_arena.revision(),
            source_binding_revision(sources.structures),
        );
        let visual_bindings = visual.binding_revision();
        let visual_revision = (
            scene.domain_visual_revision(),
            scene.attribute_revision(),
            scene.presentation_revision(),
            visual_bindings[0],
            visual_bindings[1],
            visual_bindings[2],
        );
        let dynamic_revision = (scene.structure_revision(), scene.presentation_revision());
        let state_changed = self.dynamic_state_revision != Some(dynamic_revision);
        self.dynamic_state_revision = Some(dynamic_revision);
        let data_changed = self.synced != Some(revision);
        let visual_changed = self.visual_synced != Some(visual_revision);
        if !data_changed && !visual_changed {
            if (state_changed || dynamic_sources_changed)
                && self.streams.iter().any(|stream| stream.tracks_coordinates)
            {
                self.dynamic_dirty = true;
            }
            return Ok(state_changed || dynamic_sources_changed);
        }
        let row_count = self.sync_geometry(input, data_changed)?;
        self.sync_visuals(device, queue, cull_layout, scene, visual)?;
        self.count = row_count;
        self.dynamic_dirty |= (data_changed && !self.streams.is_empty())
            || (state_changed && self.streams.iter().any(|stream| stream.tracks_coordinates));
        self.synced = Some(revision);
        self.visual_synced = Some(visual_revision);
        Ok(true)
    }

    fn sync_geometry(
        &mut self,
        input: &RelationSync<'_, D>,
        data_changed: bool,
    ) -> Result<u32, RenderError> {
        let mut output_rebound = false;
        let mut plans = Vec::new();
        if data_changed {
            self.invalidate_paged_sync();
            self.scratch.clear();
            self.visual_plans.clear();
            self.pack_legacy(input.scene, input.sources.structures)?;
            let legacy_count = count(self.scratch.len());
            if legacy_count != 0 {
                self.visual_plans.push(RelationVisualPlan {
                    source: RelationVisualSource::Fallback,
                    first: 0,
                    count: legacy_count,
                    color: molgfx_math::Rgba8::WHITE,
                    opacity: 1.0,
                });
            }
            plans = self.pack_relations(input.scene, input.picking)?;
            self.base.upload(
                input.device,
                input.queue,
                "base relation glyph table",
                &self.scratch,
            )?;
            output_rebound = self.buffer.upload(
                input.device,
                input.queue,
                "interaction glyph table",
                &self.scratch,
            )?;
        }
        let row_count = count(self.scratch.len());
        let visible_rebound = self.visible.reserve(
            input.device,
            "visible relation rows",
            u64::from(row_count).saturating_mul(4),
        )?;
        if data_changed || output_rebound || visible_rebound {
            self.cull_streams.clear();
        }
        if data_changed && plans.is_empty() {
            self.streams.clear();
        } else if data_changed {
            self.ensure_fallbacks(input.device, input.queue)?;
            self.resolvers.upload(
                input.device,
                input.queue,
                "dynamic relation resolver table",
                &self.resolver_scratch,
            )?;
        }
        if data_changed {
            write_draw_args(
                input.device,
                input.queue,
                "interaction glyph indirect arguments",
                6,
                0,
                &mut self.args,
            )?;
        }
        if output_rebound || visible_rebound || self.render_group.is_none() {
            self.bind_draw_group(input.device, input.render_layout);
        }
        if data_changed && !plans.is_empty() {
            self.bind_streams(input.device, input.resolve_layout, input.sources, &plans)?;
        }
        if data_changed {
            self.capture_scene_state();
        }
        Ok(row_count)
    }

    fn capture_scene_state(&mut self) {
        self.scene_count = self.scratch.len();
        self.scene_visual_count = self.visual_plans.len();
        self.scene_resolver_count = self.resolver_scratch.len();
        self.scene_stream_count = self.streams.len();
    }

    fn invalidate_paged_sync(&mut self) {
        self.paged_synced = None;
        self.paged_visual_synced = None;
        self.paged_timeline_synced = 0;
    }

    fn pack_legacy(
        &mut self,
        scene: &Scene,
        structures: &[GpuStructure<D>],
    ) -> Result<(), RenderError> {
        for (handle, edge) in scene.interactions().filter(|(_, edge)| edge.visible()) {
            let Some(page) = structures
                .iter()
                .find(|structure| structure.handle == edge.owner())
                .map(|structure| structure.pick_page(EntityKind::Edge))
            else {
                continue;
            };
            self.scratch.push(InteractionGpu::new(
                edge,
                u64::from(Scene::interaction_row(handle)),
                page,
            )?);
        }
        for (handle, guide) in scene.guides().filter(|(_, guide)| guide.visible()) {
            let Some(page) = structures
                .iter()
                .find(|structure| structure.handle == guide.owner())
                .map(|structure| structure.pick_page(EntityKind::Guide))
            else {
                continue;
            };
            self.scratch.push(InteractionGpu::from_guide(
                guide,
                u64::from(Scene::guide_row(handle)),
                page,
            )?);
        }
        Ok(())
    }

    fn pack_relations(
        &mut self,
        scene: &Scene,
        picking: &PickPages,
    ) -> Result<Vec<StreamPlan>, RenderError> {
        let mut streams = BTreeMap::<StreamKey, Vec<RelationResolverGpu>>::new();
        for (handle, batch) in scene
            .relation_batches()
            .filter(|(_, batch)| batch.visible())
        {
            let first = u32::try_from(self.scratch.len()).map_err(|_| row_limit())?;
            let page = picking
                .page_for_table(
                    DatasetId::new(batch.source_rows().namespace().0),
                    table_chunk(handle.row(), handle.generation()),
                    EntityKind::Relation,
                )
                .ok_or(RenderError::PickingOwnerMissing)?;
            for (logical, relation) in batch.relations().iter().copied().enumerate() {
                let logical = u32::try_from(logical).map_err(|_| row_limit())?;
                let output_row = u32::try_from(self.scratch.len()).map_err(|_| row_limit())?;
                let record = if let Some(record) = InteractionGpu::from_relation(
                    relation,
                    batch.style(),
                    u64::from(logical),
                    page,
                )? {
                    record
                } else {
                    InteractionGpu::from_relation_style(batch.style(), u64::from(logical), page)?
                };
                if matches!(
                    (relation.start, relation.end),
                    (
                        molgfx_core::SpatialAnchor::World(_),
                        molgfx_core::SpatialAnchor::World(_)
                    )
                ) {
                    self.scratch.push(record);
                    continue;
                }
                self.scratch.push(record);
                streams
                    .entry(stream_key(relation))
                    .or_default()
                    .push(RelationResolverGpu {
                        start: anchor_payload(scene, relation.start)?,
                        end: anchor_payload(scene, relation.end)?,
                        output: [output_row, logical, 0, 0],
                        reserved: [0; 4],
                    });
            }
            let end = u32::try_from(self.scratch.len()).map_err(|_| row_limit())?;
            if end != first {
                self.visual_plans.push(RelationVisualPlan {
                    source: RelationVisualSource::Scene(handle),
                    first,
                    count: end - first,
                    color: batch.style().color,
                    opacity: batch.style().opacity,
                });
            }
        }
        self.resolver_scratch.clear();
        let mut plans = Vec::with_capacity(streams.len());
        for (key, rows) in streams {
            while !self
                .resolver_scratch
                .len()
                .is_multiple_of(RESOLVER_ALIGNMENT_ROWS)
            {
                self.resolver_scratch.push(RelationResolverGpu::default());
            }
            let first = u32::try_from(self.resolver_scratch.len()).map_err(|_| row_limit())?;
            let count = u32::try_from(rows.len()).map_err(|_| row_limit())?;
            self.resolver_scratch.extend(rows);
            plans.push(StreamPlan { key, first, count });
        }
        Ok(plans)
    }

    fn bind_draw_group(&mut self, device: &D, render_layout: &D::BindGroupLayout) {
        let (Some(interactions), Some(visible)) = (self.buffer.get(), self.visible.get()) else {
            return;
        };
        self.render_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: interaction glyph table",
            layout: render_layout,
            entries: &[
                BindGroupEntry::Buffer {
                    binding: 0,
                    buffer: interactions,
                },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: visible,
                },
            ],
        }));
    }
}

include!("interaction_bindings.rs");

include!("interaction_table_views.rs");

include!("interaction_table_helpers.rs");

include!("interaction_visuals.rs");
