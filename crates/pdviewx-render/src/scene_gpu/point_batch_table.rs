//! Persistent 12-byte generic point tables with GPU-authored visibility.

use super::dispatch::workgroups_2d;
use super::generic_visual::{GenericVisualResources, GenericVisualState, GenericVisualTarget};
use super::grow_buffer::GrowBuffer;
use super::picking_pages::PickPages;
use super::visual::VisualCullEntries;
use crate::error::RenderError;
use crate::{DerivedCache, DerivedFootprint};
use pdviewx_core::{
    ChunkId, DatasetId, DrawIndirectArgs, EntityKind, PointBatch, PointBatchHandle, PointGlyph,
    Scene,
};
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue as _};
use pdviewx_math::Rgba8;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PointBatchGpu {
    source: [u32; 4],
    picking: [u32; 4],
    timeline: [f32; 4],
}

#[derive(Clone, Copy)]
pub(crate) struct GenericPointDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) source_groups: [u32; 2],
    pub(crate) tile_groups: [u32; 2],
    pub(crate) cull_visual: bool,
    pub(crate) shading_visual: bool,
}

#[derive(Debug)]
struct GpuPointBatch<D: Device> {
    handle: PointBatchHandle,
    glyph: PointGlyph,
    positions: GrowBuffer<D>,
    timeline_start: Option<D::Buffer>,
    timeline_end: Option<D::Buffer>,
    timeline_source: Option<(usize, usize)>,
    materialized: Option<D::Buffer>,
    timeline_config: Option<D::Buffer>,
    timeline_group: Option<D::BindGroup>,
    output: D::Buffer,
    config: D::Buffer,
    cull_group: D::BindGroup,
    render_group: D::BindGroup,
    visual: GenericVisualState<D>,
    count: u32,
}

pub(super) struct PointBatchSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) cull_layout: &'a D::BindGroupLayout,
    pub(super) render_layout: &'a D::BindGroupLayout,
    pub(super) timeline_layout: &'a D::BindGroupLayout,
    pub(super) scene: &'a Scene,
    pub(super) picking: &'a PickPages,
    pub(super) extent: [u32; 2],
    pub(super) visual: GenericVisualResources<'a, D>,
    pub(super) derived_cache: &'a mut DerivedCache,
    pub(super) frame: u64,
}

struct PointTiles<'a, D: Device> {
    buffer: &'a D::Buffer,
    extent: [u32; 2],
    count: u32,
}

struct PointGroupContext<'a, D: Device> {
    device: &'a D,
    cull_layout: &'a D::BindGroupLayout,
    render_layout: &'a D::BindGroupLayout,
    tiles: &'a D::Buffer,
}

struct PointGroupInputs<'a, D: Device> {
    positions: &'a D::Buffer,
    frame_end: &'a D::Buffer,
    output: &'a D::Buffer,
    config: &'a D::Buffer,
    visual: VisualCullEntries<'a, D>,
}

struct PointGroups<D: Device> {
    cull: D::BindGroup,
    render: D::BindGroup,
}

type PointSyncRevision = (u64, u64, u64, u64, u64, u64, u64, u64, [u32; 2]);

#[derive(Debug)]
pub(super) struct GpuPointBatches<D: Device> {
    batches: Vec<GpuPointBatch<D>>,
    tiles: GrowBuffer<D>,
    tile_extent: [u32; 2],
    synced: Option<PointSyncRevision>,
}

impl<D: Device> GpuPointBatches<D> {
    pub(super) const fn new() -> Self {
        Self {
            batches: Vec::new(),
            tiles: GrowBuffer::new(),
            tile_extent: [0; 2],
            synced: None,
        }
    }

    pub(super) fn sync(
        &mut self,
        context: &mut PointBatchSync<'_, D>,
    ) -> Result<bool, RenderError> {
        let device = context.device;
        let cull_layout = context.cull_layout;
        let render_layout = context.render_layout;
        let scene = context.scene;
        let picking = context.picking;
        let extent = context.extent;
        let visual_resources = context.visual;
        let tile_extent = [extent[0].max(1).div_ceil(8), extent[1].max(1).div_ceil(8)];
        let tile_count = tile_extent[0]
            .checked_mul(tile_extent[1])
            .ok_or_else(tile_limit)?;
        let tiles_rebound = self.tiles.reserve(
            device,
            "generic point screen tiles",
            u64::from(tile_count) * 4,
        )?;
        self.tile_extent = tile_extent;
        let revision = (
            scene.cache_identity(),
            scene.generic_batch_revision(),
            scene.domain_visual_revision(),
            scene.presentation_revision(),
            picking.scene_table_revision(),
            visual_resources.programs.binding_revision(),
            visual_resources.parameters.binding_revision(),
            visual_resources.properties.binding_revision(),
            extent,
        );
        if self.synced == Some(revision) {
            return Ok(false);
        }
        Self::plan_timelines(context);
        let before = self.batches.len();
        self.batches.retain(|entry| {
            scene
                .point_batch(entry.handle)
                .is_some_and(PointBatch::visible)
        });
        let mut changed = self.batches.len() != before || tiles_rebound;
        if tiles_rebound {
            let Some(tiles) = self.tiles.get() else {
                return Err(missing_buffer());
            };
            for entry in &mut self.batches {
                entry.rebind(device, cull_layout, render_layout, tiles, visual_resources);
            }
        }
        let tiles = PointTiles {
            buffer: self.tiles.get().ok_or_else(missing_buffer)?,
            extent: tile_extent,
            count: tile_count,
        };
        for (handle, batch) in scene.point_batches().filter(|(_, batch)| batch.visible()) {
            let page = point_page(picking, handle, batch)?;
            changed |= Self::sync_batch(&mut self.batches, context, handle, batch, page, &tiles)?;
        }
        self.synced = Some(revision);
        Ok(changed)
    }

    fn sync_batch(
        batches: &mut Vec<GpuPointBatch<D>>,
        context: &PointBatchSync<'_, D>,
        handle: PointBatchHandle,
        batch: &PointBatch,
        page: u32,
        tiles: &PointTiles<'_, D>,
    ) -> Result<bool, RenderError> {
        let index = match batches.binary_search_by_key(&handle, |entry| entry.handle) {
            Ok(index) => index,
            Err(index) => {
                let entry = GpuPointBatch::new(context, handle, batch, page, tiles)?;
                batches.insert(index, entry);
                return Ok(true);
            }
        };
        write_config::<D>(
            context.queue,
            batch,
            &PointConfigWrite {
                buffer: &batches[index].config,
                page,
                tile_extent: tiles.extent,
                tile_count: tiles.count,
                frames: context.scene.point_frames(handle),
                materialized: batches[index].materialized.is_some(),
            },
        );
        let timeline_changed = batches[index].sync_timeline(context)?;
        let visual_changed = batches[index].visual.sync(
            context.device,
            context.queue,
            &GenericVisualTarget {
                scene: context.scene,
                domain: pdviewx_core::RowDomain::Points(handle),
                color: batch.style().color,
                opacity: f32::from(batch.style().color.a) / 255.0,
                row_count: count(batch),
            },
            context.visual,
        )?;
        if visual_changed || timeline_changed {
            batches[index].rebind(
                context.device,
                context.cull_layout,
                context.render_layout,
                tiles.buffer,
                context.visual,
            );
        }
        Ok(visual_changed || timeline_changed)
    }

    pub(super) fn dispatches(&self) -> impl Iterator<Item = GenericPointDispatch<'_, D>> {
        self.batches.iter().map(|batch| GenericPointDispatch {
            group: &batch.cull_group,
            source_groups: workgroups_2d(u64::from(batch.count).div_ceil(64)),
            tile_groups: workgroups_2d(
                u64::from(self.tile_extent[0])
                    .saturating_mul(u64::from(self.tile_extent[1]))
                    .div_ceil(64),
            ),
            cull_visual: batch.visual.has_cull_results(),
            shading_visual: batch.visual.has_shading_results(),
        })
    }

    pub(super) fn draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, super::SlotShading)> {
        self.batches
            .iter()
            .filter(move |batch| batch.visual.is_translucent() == translucent)
            .map(|batch| (&batch.render_group, &batch.output, batch.visual.shading()))
    }

    pub(super) fn has_translucency(&self) -> bool {
        self.batches
            .iter()
            .any(|batch| batch.visual.is_translucent())
    }

    pub(super) fn has_visible(&self) -> bool {
        !self.batches.is_empty()
    }

    pub(super) fn is_massive_opaque_discs(&self) -> bool {
        const MASSIVE_POINT_COUNT: u64 = 131_072;
        !self.batches.is_empty()
            && self
                .batches
                .iter()
                .all(|batch| batch.glyph == PointGlyph::Disc && !batch.visual.is_translucent())
            && self
                .batches
                .iter()
                .map(|batch| u64::from(batch.count))
                .sum::<u64>()
                >= MASSIVE_POINT_COUNT
    }

    pub(super) fn relation_source_entries(
        &self,
        handle: PointBatchHandle,
        bindings: [u32; 3],
    ) -> Option<[BindGroupEntry<'_, D>; 3]> {
        let index = self
            .batches
            .binary_search_by_key(&handle, |entry| entry.handle)
            .ok()?;
        let batch = &self.batches[index];
        Some([
            BindGroupEntry::Buffer {
                binding: bindings[0],
                buffer: batch.active_start_buffer()?,
            },
            BindGroupEntry::Buffer {
                binding: bindings[1],
                buffer: batch.active_end_buffer()?,
            },
            BindGroupEntry::Buffer {
                binding: bindings[2],
                buffer: &batch.config,
            },
        ])
    }

    pub(super) fn timeline_dispatches(
        &self,
    ) -> impl Iterator<Item = GenericPointTimelineDispatch<'_, D>> {
        self.batches.iter().filter_map(|batch| {
            batch
                .timeline_group
                .as_ref()
                .map(|group| GenericPointTimelineDispatch {
                    group,
                    groups: workgroups_2d(u64::from(batch.count).div_ceil(64)),
                })
        })
    }
}

include!("point_batch_resource.rs");
include!("point_batch_timeline.rs");

include!("point_batch_groups.rs");

fn count(batch: &PointBatch) -> usize {
    batch.source_rows().len() as usize
}

fn point_page(
    picking: &PickPages,
    handle: PointBatchHandle,
    batch: &PointBatch,
) -> Result<u32, RenderError> {
    let dataset = DatasetId::new(batch.source_rows().namespace().0);
    let chunk = table_chunk(handle);
    picking
        .page_for_table(dataset, chunk, EntityKind::Point)
        .ok_or(RenderError::PickingOwnerMissing)
}

struct PointConfigWrite<'a, D: Device> {
    buffer: &'a D::Buffer,
    page: u32,
    tile_extent: [u32; 2],
    tile_count: u32,
    frames: Option<pdviewx_core::PointFramePair<'a>>,
    materialized: bool,
}

fn write_config<D: Device>(queue: &D::Queue, batch: &PointBatch, target: &PointConfigWrite<'_, D>) {
    let style = batch.style();
    queue.write_buffer(
        target.buffer,
        0,
        bytemuck::bytes_of(&PointBatchGpu {
            source: [
                batch.source_rows().len(),
                style.radius.to_bits(),
                pack_color(style.color),
                batch.glyph() as u32,
            ],
            picking: [
                target.page,
                target.tile_extent[0],
                target.tile_count,
                u32::from(batch.source_rows().len() <= 0x00ff_fffe),
            ],
            timeline: [
                if target.materialized {
                    0.0
                } else {
                    target.frames.map_or(0.0, |(_, _, alpha, _)| alpha)
                },
                0.0,
                0.0,
                0.0,
            ],
        }),
    );
}

const fn pack_color(color: Rgba8) -> u32 {
    color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16) | ((color.a as u32) << 24)
}

fn table_chunk(handle: PointBatchHandle) -> ChunkId {
    ChunkId::new(u64::from(handle.row()) | (u64::from(handle.generation()) << 32))
}

fn missing_buffer() -> RenderError {
    pdviewx_gpu::GpuError::LimitExceeded {
        resource: "generic point buffer",
        limit: 0,
    }
    .into()
}

fn tile_limit() -> RenderError {
    pdviewx_gpu::GpuError::LimitExceeded {
        resource: "generic point screen tiles",
        limit: u64::from(u32::MAX),
    }
    .into()
}
