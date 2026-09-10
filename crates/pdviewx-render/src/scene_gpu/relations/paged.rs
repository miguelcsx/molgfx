//! Paged relation lowering onto the shared glyph and resolver pipelines.

use super::{
    BTreeMap, BindGroupDesc, BindGroupEntry, Device, GenericVisualResources, GpuInstanceBatches,
    GpuInteractions, InteractionGpu, PagedRelationVisualState, PickPages, RESOLVER_ALIGNMENT_ROWS,
    RelationResolverGpu, RelationStream, RelationVisualPlan, RelationVisualSource, RenderError,
    count, row_limit, workgroups_2d, write_draw_args,
};
use crate::engine::chunk_draw_plan::{ResidentRelationChunkPlacement, ResidentSpatialAnchor};
use pdviewx_core::{EntityKind, PagedSpatialAnchor};

#[path = "paged/support.rs"]
mod support;
use support::{
    anchor_key, create_model, create_rigid_config, missing_paged_source, paged_anchor_payload,
    paged_pipeline, rigid_timeline, source_entry, tracks_coordinates, write_matching_rigid_alpha,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PagedAnchorKey {
    World,
    Position {
        source: u8,
        byte_offset: u64,
        byte_len: u64,
        model: [u32; 16],
    },
    Rigid {
        start_byte_offset: u64,
        end_byte_offset: u64,
        byte_len: u64,
        interpolation: u32,
    },
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PagedRigidConfig {
    counts: [u32; 4],
    picking_style: [u32; 4],
    bounds: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PagedStreamKey {
    start: PagedAnchorKey,
    end: PagedAnchorKey,
}

struct PagedRelationSources<'a, D: Device> {
    display: &'a D::Buffer,
    generic: &'a D::Buffer,
    instances: &'a GpuInstanceBatches<D>,
}

impl<D: Device> Copy for PagedRelationSources<'_, D> {}

impl<D: Device> Clone for PagedRelationSources<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: Device> GpuInteractions<D> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::scene_gpu) fn sync_paged<F, A>(
        &mut self,
        device: &D,
        queue: &D::Queue,
        render_layout: &D::BindGroupLayout,
        cull_layout: &D::BindGroupLayout,
        resolve_layout: &D::BindGroupLayout,
        display_source: &D::Buffer,
        generic_source: &D::Buffer,
        instance_sources: &GpuInstanceBatches<D>,
        plans: &[ResidentRelationChunkPlacement],
        instance_plans: &[crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
        picking: &PickPages,
        resources: GenericVisualResources<'_, D>,
        revision: u64,
        source_revision: u64,
        instance_binding_revision: u64,
        instance_timeline_revision: u64,
        attribute_timeline_revision: u64,
        visual_revision: u64,
        mut resolve: F,
        mut resolve_attribute: A,
    ) -> Result<bool, RenderError>
    where
        F: FnMut(PagedSpatialAnchor) -> Option<ResidentSpatialAnchor>,
        A: FnMut(
            pdviewx_core::ResidencyTicket,
        ) -> Option<crate::engine::chunk_draw_plan::ResidentAttributeColumn>,
    {
        let sync_key = (
            revision,
            source_revision,
            picking.table_revision(),
            instance_binding_revision,
        );
        let visual_key = (revision, attribute_timeline_revision, visual_revision);
        let geometry_changed = self.paged_synced != Some(sync_key);
        let visual_changed = self.paged_visual_synced != Some(visual_key);
        let timeline_changed = self.paged_timeline_synced != instance_timeline_revision;
        if !geometry_changed && !visual_changed && !timeline_changed {
            return Ok(false);
        }
        if geometry_changed {
            self.rebuild_paged_geometry(
                device,
                queue,
                render_layout,
                resolve_layout,
                display_source,
                generic_source,
                instance_sources,
                plans,
                picking,
                &mut resolve,
            )?;
            self.cull_streams.clear();
            self.paged_synced = Some(sync_key);
        }
        let visual_resources_changed = self.sync_paged_visuals(
            device,
            queue,
            plans,
            resources.time_seconds,
            &mut resolve_attribute,
        )?;
        if geometry_changed || visual_resources_changed {
            self.rebuild_cull_streams(device, queue, cull_layout, resources, Some(generic_source))?;
        }
        if timeline_changed {
            self.sync_paged_rigid_timelines(queue, instance_plans);
            self.paged_timeline_synced = instance_timeline_revision;
            self.dynamic_dirty = true;
        }
        self.paged_visual_synced = Some(visual_key);
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn rebuild_paged_geometry<F>(
        &mut self,
        device: &D,
        queue: &D::Queue,
        render_layout: &D::BindGroupLayout,
        resolve_layout: &D::BindGroupLayout,
        display_source: &D::Buffer,
        generic_source: &D::Buffer,
        instance_sources: &GpuInstanceBatches<D>,
        plans: &[ResidentRelationChunkPlacement],
        picking: &PickPages,
        resolve: &mut F,
    ) -> Result<(), RenderError>
    where
        F: FnMut(PagedSpatialAnchor) -> Option<ResidentSpatialAnchor>,
    {
        self.truncate_paged_state();
        self.ensure_fallbacks(device, queue)?;
        let mut streams = BTreeMap::<PagedStreamKey, Vec<RelationResolverGpu>>::new();
        for plan in plans {
            self.pack_paged_plan(plan, picking, &mut streams, resolve)?;
        }
        let stream_plans = self.append_paged_resolvers(streams);
        self.base
            .upload(device, queue, "base relation glyph table", &self.scratch)?;
        let output_rebound =
            self.buffer
                .upload(device, queue, "interaction glyph table", &self.scratch)?;
        let row_count = count(self.scratch.len());
        let visible_rebound = self.visible.reserve(
            device,
            "visible relation rows",
            u64::from(row_count).saturating_mul(4),
        )?;
        self.resolvers.upload(
            device,
            queue,
            "dynamic relation resolver table",
            &self.resolver_scratch,
        )?;
        write_draw_args(
            device,
            queue,
            "interaction glyph indirect arguments",
            6,
            0,
            &mut self.args,
        )?;
        if output_rebound || visible_rebound || self.render_group.is_none() {
            self.bind_draw_group(device, render_layout);
        }
        self.bind_paged_streams(
            device,
            queue,
            resolve_layout,
            PagedRelationSources {
                display: display_source,
                generic: generic_source,
                instances: instance_sources,
            },
            &stream_plans,
        )?;
        self.count = row_count;
        self.dynamic_dirty |= !stream_plans.is_empty();
        Ok(())
    }

    fn sync_paged_visuals<A>(
        &mut self,
        device: &D,
        queue: &D::Queue,
        plans: &[ResidentRelationChunkPlacement],
        time_seconds: f32,
        resolve: &mut A,
    ) -> Result<bool, RenderError>
    where
        A: FnMut(
            pdviewx_core::ResidencyTicket,
        ) -> Option<crate::engine::chunk_draw_plan::ResidentAttributeColumn>,
    {
        let mut changed = self.paged_visual_arenas.sync(device, queue, plans)?;
        self.paged_visuals.retain(|id, _| {
            plans
                .iter()
                .any(|plan| plan.id == *id && plan.visual.is_some())
        });
        for plan in plans {
            let Some(descriptor) = plan.visual.as_ref() else {
                continue;
            };
            let descriptor = std::sync::Arc::clone(descriptor);
            let state = self.paged_visuals.entry(plan.id).or_insert_with(|| {
                PagedRelationVisualState::new(std::sync::Arc::clone(&descriptor))
            });
            changed |= state.sync(
                device,
                queue,
                descriptor,
                plan.relations.len(),
                plan.style.color,
                plan.style.opacity,
                time_seconds,
                &self.paged_visual_arenas,
                &mut *resolve,
            )?;
        }
        Ok(changed)
    }

    fn truncate_paged_state(&mut self) {
        self.scratch.truncate(self.scene_count);
        self.visual_plans.truncate(self.scene_visual_count);
        self.resolver_scratch.truncate(self.scene_resolver_count);
        self.streams.truncate(self.scene_stream_count);
    }

    fn pack_paged_plan<F>(
        &mut self,
        plan: &ResidentRelationChunkPlacement,
        picking: &PickPages,
        streams: &mut BTreeMap<PagedStreamKey, Vec<RelationResolverGpu>>,
        resolve: &mut F,
    ) -> Result<(), RenderError>
    where
        F: FnMut(PagedSpatialAnchor) -> Option<ResidentSpatialAnchor>,
    {
        let page = picking
            .page_for_chunk(
                plan.ticket.key.dataset,
                plan.ticket.key.chunk,
                EntityKind::Relation,
            )
            .ok_or(RenderError::PickingOwnerMissing)?;
        let first = count(self.scratch.len());
        for (local, relation) in plan.relations.iter().copied().enumerate() {
            let local = u32::try_from(local).map_err(|_| row_limit())?;
            let output = count(self.scratch.len());
            let mut glyph =
                InteractionGpu::from_relation_style(plan.style, u64::from(local), page)?;
            let start = resolve(relation.start).ok_or_else(missing_paged_source)?;
            let end = resolve(relation.end).ok_or_else(missing_paged_source)?;
            if let (ResidentSpatialAnchor::World(start), ResidentSpatialAnchor::World(end)) =
                (start, end)
            {
                glyph.start_width[..3].copy_from_slice(&start.to_array());
                glyph.end_period[..3].copy_from_slice(&end.to_array());
            } else {
                streams
                    .entry(PagedStreamKey {
                        start: anchor_key(start),
                        end: anchor_key(end),
                    })
                    .or_default()
                    .push(RelationResolverGpu {
                        start: paged_anchor_payload(start),
                        end: paged_anchor_payload(end),
                        output: [output, local, 0, 0],
                        reserved: [0; 4],
                    });
            }
            self.scratch.push(glyph);
        }
        let count = count(self.scratch.len()).saturating_sub(first);
        if count != 0 {
            self.visual_plans.push(RelationVisualPlan {
                source: plan
                    .visual
                    .as_ref()
                    .map_or(RelationVisualSource::Fallback, |_| {
                        RelationVisualSource::Paged(plan.id)
                    }),
                first,
                count,
                color: plan.style.color,
                opacity: plan.style.opacity,
            });
        }
        Ok(())
    }

    fn append_paged_resolvers(
        &mut self,
        streams: BTreeMap<PagedStreamKey, Vec<RelationResolverGpu>>,
    ) -> Vec<(PagedStreamKey, u32, u32)> {
        let mut plans = Vec::with_capacity(streams.len());
        for (key, rows) in streams {
            while !self
                .resolver_scratch
                .len()
                .is_multiple_of(RESOLVER_ALIGNMENT_ROWS)
            {
                self.resolver_scratch.push(RelationResolverGpu::default());
            }
            let first = count(self.resolver_scratch.len());
            let row_count = count(rows.len());
            self.resolver_scratch.extend(rows);
            plans.push((key, first, row_count));
        }
        plans
    }

    fn bind_paged_streams(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        sources: PagedRelationSources<'_, D>,
        plans: &[(PagedStreamKey, u32, u32)],
    ) -> Result<(), RenderError> {
        let resolver = self.resolvers.get().ok_or_else(missing_paged_source)?;
        let output = self.buffer.get().ok_or_else(missing_paged_source)?;
        let fallback_source = self
            .source_fallback
            .as_ref()
            .ok_or_else(missing_paged_source)?;
        let fallback_model = self
            .model_fallback
            .as_ref()
            .ok_or_else(missing_paged_source)?;
        let fallback_config = self
            .timeline_fallback
            .as_ref()
            .ok_or_else(missing_paged_source)?;
        for (key, first, row_count) in plans.iter().copied() {
            let start_model = create_model(device, queue, key.start)?;
            let end_model = create_model(device, queue, key.end)?;
            let start_config = create_rigid_config(device, queue, key.start)?;
            let end_config = create_rigid_config(device, queue, key.end)?;
            let start_model_ref = match &start_model {
                Some(model) => model,
                None => fallback_model,
            };
            let end_model_ref = match &end_model {
                Some(model) => model,
                None => fallback_model,
            };
            let start_config_ref = match &start_config {
                Some(config) => config,
                None => fallback_config,
            };
            let end_config_ref = match &end_config {
                Some(config) => config,
                None => fallback_config,
            };
            let entries = [
                BindGroupEntry::BufferRange {
                    binding: 0,
                    buffer: resolver,
                    offset: u64::from(first) * std::mem::size_of::<RelationResolverGpu>() as u64,
                    size: u64::from(row_count) * std::mem::size_of::<RelationResolverGpu>() as u64,
                },
                source_entry(key.start, 1, false, sources, fallback_source),
                source_entry(key.end, 2, false, sources, fallback_source),
                BindGroupEntry::Buffer {
                    binding: 3,
                    buffer: output,
                },
                BindGroupEntry::Buffer {
                    binding: 4,
                    buffer: start_model_ref,
                },
                BindGroupEntry::Buffer {
                    binding: 5,
                    buffer: end_model_ref,
                },
                source_entry(key.start, 6, true, sources, fallback_source),
                source_entry(key.end, 7, true, sources, fallback_source),
                BindGroupEntry::Buffer {
                    binding: 8,
                    buffer: start_config_ref,
                },
                BindGroupEntry::Buffer {
                    binding: 9,
                    buffer: end_config_ref,
                },
            ];
            self.streams.push(RelationStream {
                pipeline: paged_pipeline(key),
                tracks_coordinates: tracks_coordinates(key.start) || tracks_coordinates(key.end),
                groups: workgroups_2d(u64::from(row_count).div_ceil(64)),
                group: device.create_bind_group(&BindGroupDesc {
                    label: "group1: paged relation resolver",
                    layout,
                    entries: &entries,
                }),
                _start_model: start_model,
                _end_model: end_model,
                start_timeline: rigid_timeline(key.start),
                end_timeline: rigid_timeline(key.end),
                start_config,
                end_config,
            });
        }
        Ok(())
    }
}

impl<D: Device> GpuInteractions<D> {
    fn sync_paged_rigid_timelines(
        &mut self,
        queue: &D::Queue,
        plans: &[crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
    ) {
        for stream in &mut self.streams[self.scene_stream_count..] {
            write_matching_rigid_alpha::<D>(
                queue,
                stream.start_timeline,
                stream.start_config.as_ref(),
                plans,
            );
            write_matching_rigid_alpha::<D>(
                queue,
                stream.end_timeline,
                stream.end_config.as_ref(),
                plans,
            );
        }
    }
}
