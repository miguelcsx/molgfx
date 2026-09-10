//! Shared analytic templates and compact 32-byte rigid-instance GPU tables.
use super::dispatch::workgroups_2d;
use super::generic_visual::{GenericVisualResources, GenericVisualState, GenericVisualTarget};
use super::picking_pages::PickPages;
use super::visual::VisualCullEntries;
use crate::error::RenderError;
use crate::{DerivedCache, DerivedFootprint};
use pdviewx_core::{
    AnalyticTemplate, DrawIndirectArgs, EntityKind, InstanceBatch, InstanceBatchHandle, Scene,
};
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue as _};
use pdviewx_math::{Quat, Vec3};
use std::sync::Arc;
#[path = "instance_batch_table/support.rs"]
mod support;
use support::{
    checked_count, frame_identity, limit, pack_color, storage_buffer, template_part_page,
};
pub(crate) const GENERIC_INSTANCE_SPHERE: u32 = 0;
pub(crate) const GENERIC_INSTANCE_CAPSULE: u32 = 3;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GenericInstanceConfig {
    counts: [u32; 4],
    picking_style: [u32; 4],
    bounds: [f32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceTimelineConfig {
    values: [f32; 4],
    counts: [u32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GenericCapsuleGpu {
    center_length: [f32; 4],
    orientation: [f32; 4],
    radius_reserved: [f32; 4],
}

#[derive(Debug)]
struct GpuAnalyticTemplate<D: Device> {
    source: Arc<AnalyticTemplate>,
    spheres: D::Buffer,
    capsules: D::Buffer,
}

struct InstanceGroupContext<'a, D: Device> {
    device: &'a D,
    cull_layout: &'a D::BindGroupLayout,
    render_layout: &'a D::BindGroupLayout,
}

struct InstanceGroupInputs<'a, D: Device> {
    template: &'a GpuAnalyticTemplate<D>,
    transforms: &'a D::Buffer,
    frame_start: &'a D::Buffer,
    frame_end: &'a D::Buffer,
    args: &'a D::Buffer,
    config: &'a D::Buffer,
    visual: VisualCullEntries<'a, D>,
}

struct InstanceGroups<D: Device> {
    cull: D::BindGroup,
    render: D::BindGroup,
}

type MaterializationResources<D> = (
    Option<<D as Device>::Buffer>,
    Option<<D as Device>::Buffer>,
    Option<<D as Device>::BindGroup>,
);

#[derive(Debug)]
struct GpuInstanceBatch<D: Device> {
    handle: InstanceBatchHandle,
    template: Arc<GpuAnalyticTemplate<D>>,
    transforms: D::Buffer,
    timeline_start: Option<D::Buffer>,
    timeline_end: Option<D::Buffer>,
    timeline_source: Option<(usize, usize)>,
    materialized: Option<D::Buffer>,
    timeline_config: Option<D::Buffer>,
    timeline_group: Option<D::BindGroup>,
    args: D::Buffer,
    config: D::Buffer,
    cull_group: D::BindGroup,
    render_group: D::BindGroup,
    visual: GenericVisualState<D>,
    count: u32,
    sphere_count: u32,
    capsule_count: u32,
}

include!("instance_batch_paged.rs");

pub(super) struct InstanceBatchSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) cull_layout: &'a D::BindGroupLayout,
    pub(super) render_layout: &'a D::BindGroupLayout,
    pub(super) timeline_layout: &'a D::BindGroupLayout,
    pub(super) scene: &'a Scene,
    pub(super) picking: &'a PickPages,
    pub(super) visual: GenericVisualResources<'a, D>,
    pub(super) derived_cache: &'a mut DerivedCache,
    pub(super) frame: u64,
}

type InstanceSyncRevision = (u64, u64, u64, u64, u64, u64, u64, u64);

#[derive(Clone, Copy)]
pub(crate) struct GenericInstanceDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) groups: [u32; 2],
    pub(crate) cull_visual: bool,
    pub(crate) shading_visual: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct GenericInstanceTimelineDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) groups: [u32; 2],
}

#[derive(Clone, Copy)]
pub(crate) struct GenericInstanceDraw<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) args: &'a D::Buffer,
    pub(crate) args_offset: u64,
    pub(crate) shape: u32,
    pub(crate) shading: super::SlotShading,
}

#[derive(Debug)]
pub(super) struct GpuInstanceBatches<D: Device> {
    batches: Vec<GpuInstanceBatch<D>>,
    paged: Vec<GpuPagedInstanceBatch<D>>,
    paged_materialization: Vec<bool>,
    paged_binding_revision: u64,
    synced: Option<InstanceSyncRevision>,
}

impl<D: Device> GpuInstanceBatches<D> {
    pub(super) const fn new() -> Self {
        Self {
            batches: Vec::new(),
            paged: Vec::new(),
            paged_materialization: Vec::new(),
            paged_binding_revision: 0,
            synced: None,
        }
    }

    pub(super) fn sync(
        &mut self,
        context: &mut InstanceBatchSync<'_, D>,
    ) -> Result<bool, RenderError> {
        let device = context.device;
        let queue = context.queue;
        let cull_layout = context.cull_layout;
        let render_layout = context.render_layout;
        let scene = context.scene;
        let picking = context.picking;
        let visual_resources = context.visual;
        let revision = (
            scene.cache_identity(),
            scene.generic_batch_revision(),
            scene.domain_visual_revision(),
            scene.presentation_revision(),
            picking.scene_table_revision(),
            visual_resources.programs.binding_revision(),
            visual_resources.parameters.binding_revision(),
            visual_resources.properties.binding_revision(),
        );
        if self.synced == Some(revision) {
            return Ok(false);
        }
        for entry in &self.batches {
            if !scene.instance_batch(entry.handle).is_some_and(|batch| {
                batch.visible() && scene.instance_frames(entry.handle).is_some()
            }) {
                context
                    .derived_cache
                    .release(timeline_cache_key(entry.handle));
            }
        }
        plan_timelines(context);
        let before = self.batches.len();
        self.batches.retain(|entry| {
            scene
                .instance_batch(entry.handle)
                .is_some_and(InstanceBatch::visible)
        });
        let mut changed = before != self.batches.len();
        for (handle, batch) in scene
            .instance_batches()
            .filter(|(_, batch)| batch.visible())
        {
            let page = template_part_page(picking, handle, batch)?;
            match self
                .batches
                .binary_search_by_key(&handle, |entry| entry.handle)
            {
                Ok(index) => {
                    let timeline_changed = self.batches[index].sync_timeline(context, batch)?;
                    write_config::<D>(
                        queue,
                        &self.batches[index].config,
                        batch,
                        page,
                        scene.instance_frames(handle),
                        self.batches[index].materialized.is_some(),
                    )?;
                    let visual_changed = self.batches[index].visual.sync(
                        device,
                        queue,
                        &GenericVisualTarget {
                            scene,
                            domain: pdviewx_core::RowDomain::Instances(handle),
                            color: batch.style().color,
                            opacity: f32::from(batch.style().color.a) / 255.0,
                            row_count: batch.source_rows().len() as usize,
                        },
                        visual_resources,
                    )?;
                    if visual_changed || timeline_changed {
                        self.batches[index].rebind(
                            device,
                            cull_layout,
                            render_layout,
                            visual_resources,
                        );
                    }
                    changed |= visual_changed || timeline_changed;
                }
                Err(index) => {
                    let template = self
                        .batches
                        .iter()
                        .find(|entry| Arc::ptr_eq(&entry.template.source, batch.template()))
                        .map(|entry| Arc::clone(&entry.template))
                        .map_or_else(
                            || GpuAnalyticTemplate::new(device, queue, batch.template()),
                            Ok,
                        )?;
                    let entry = GpuInstanceBatch::new(context, handle, batch, page, template)?;
                    self.batches.insert(index, entry);
                    changed = true;
                }
            }
        }
        self.synced = Some(revision);
        Ok(changed)
    }

    pub(super) fn dispatches(&self) -> impl Iterator<Item = GenericInstanceDispatch<'_, D>> {
        self.batches
            .iter()
            .map(instance_dispatch)
            .chain(self.paged.iter().map(paged_instance_dispatch))
    }

    pub(super) fn timeline_dispatches(
        &self,
    ) -> impl Iterator<Item = GenericInstanceTimelineDispatch<'_, D>> {
        self.batches
            .iter()
            .filter_map(|batch| {
                Some(GenericInstanceTimelineDispatch {
                    group: batch.timeline_group.as_ref()?,
                    groups: workgroups_2d(u64::from(batch.count).div_ceil(64)),
                })
            })
            .chain(self.paged.iter().filter_map(|batch| {
                Some(GenericInstanceTimelineDispatch {
                    group: batch.timeline_group.as_ref()?,
                    groups: workgroups_2d(u64::from(batch.count).div_ceil(64)),
                })
            }))
    }

    pub(super) fn draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = GenericInstanceDraw<'_, D>> {
        let resident = self
            .batches
            .iter()
            .filter(move |batch| batch.visual.is_translucent() == translucent)
            .flat_map(|batch| {
                [
                    (batch.sphere_count != 0).then_some(GenericInstanceDraw {
                        group: &batch.render_group,
                        args: &batch.args,
                        args_offset: 0,
                        shape: GENERIC_INSTANCE_SPHERE,
                        shading: batch.visual.shading(),
                    }),
                    (batch.capsule_count != 0).then_some(GenericInstanceDraw {
                        group: &batch.render_group,
                        args: &batch.args,
                        args_offset: std::mem::size_of::<DrawIndirectArgs>() as u64,
                        shape: GENERIC_INSTANCE_CAPSULE,
                        shading: batch.visual.shading(),
                    }),
                ]
                .into_iter()
                .flatten()
            });
        resident.chain(
            self.paged
                .iter()
                .filter(move |batch| batch.visual.is_translucent() == translucent)
                .flat_map(paged_instance_draws),
        )
    }

    pub(super) fn has_translucency(&self) -> bool {
        self.batches
            .iter()
            .any(|batch| batch.visual.is_translucent())
            || self.paged.iter().any(|batch| batch.visual.is_translucent())
    }

    pub(super) fn has_visible(&self) -> bool {
        !self.batches.is_empty() || !self.paged.is_empty()
    }
}

fn instance_dispatch<D: Device>(batch: &GpuInstanceBatch<D>) -> GenericInstanceDispatch<'_, D> {
    GenericInstanceDispatch {
        group: &batch.cull_group,
        groups: workgroups_2d(u64::from(batch.count).div_ceil(64)),
        cull_visual: batch.visual.has_cull_results(),
        shading_visual: batch.visual.has_shading_results(),
    }
}

include!("instance_batch_relations.rs");

fn plan_timelines<D: Device>(context: &mut InstanceBatchSync<'_, D>) {
    for (handle, batch) in context
        .scene
        .instance_batches()
        .filter(|(_, batch)| batch.visible())
    {
        let key = timeline_cache_key(handle);
        if context.scene.instance_frames(handle).is_none() {
            context.derived_cache.release(key);
            continue;
        }
        let footprint = DerivedFootprint {
            cpu_bytes: 0,
            gpu_bytes: u64::from(batch.source_rows().len()).saturating_mul(32),
        };
        let _plan = context.derived_cache.plan(key, footprint, 2, context.frame);
    }
    let _evicted_count = context.derived_cache.take_evictions().count();
}

fn timeline_materialized<D: Device>(
    context: &InstanceBatchSync<'_, D>,
    handle: InstanceBatchHandle,
) -> bool {
    context.derived_cache.contains(timeline_cache_key(handle))
}

fn timeline_cache_key(handle: InstanceBatchHandle) -> crate::engine::DerivedCacheKey {
    crate::engine::DerivedCacheKey::SceneInstance(handle)
}

impl<D: Device> GpuAnalyticTemplate<D> {
    fn new(
        device: &D,
        queue: &D::Queue,
        source: &Arc<AnalyticTemplate>,
    ) -> Result<Arc<Self>, RenderError> {
        let spheres = storage_buffer(
            device,
            queue,
            "generic analytic template spheres",
            source.spheres().as_ref(),
        )?;
        let capsule_values = source
            .capsules()
            .iter()
            .map(|capsule| {
                let axis = capsule.end() - capsule.start();
                let length = axis.length();
                let center = (capsule.start() + capsule.end()) * 0.5;
                GenericCapsuleGpu {
                    center_length: [center.x, center.y, center.z, length],
                    orientation: Quat::from_rotation_arc(Vec3::Z, axis / length).to_array(),
                    radius_reserved: [capsule.radius(), 0.0, 0.0, 0.0],
                }
            })
            .collect::<Vec<_>>();
        let capsules = storage_buffer(
            device,
            queue,
            "generic analytic template capsules",
            &capsule_values,
        )?;
        Ok(Arc::new(Self {
            source: Arc::clone(source),
            spheres,
            capsules,
        }))
    }
}

include!("instance_batch_resource.rs");

include!("instance_batch_groups.rs");

fn write_config<D: Device>(
    queue: &D::Queue,
    buffer: &D::Buffer,
    batch: &InstanceBatch,
    page: u32,
    timeline: Option<pdviewx_core::InstanceFramePair<'_>>,
    materialized: bool,
) -> Result<(), RenderError> {
    let spheres = checked_count(batch.template().spheres().len())?;
    let capsules = checked_count(batch.template().capsules().len())?;
    let parts = spheres.checked_add(capsules).ok_or_else(limit)?;
    queue.write_buffer(
        buffer,
        0,
        bytemuck::bytes_of(&GenericInstanceConfig {
            counts: [batch.source_rows().len(), spheres, capsules, parts],
            picking_style: [page, pack_color(batch.style().color), 0, 0],
            bounds: [
                batch.template().bound_radius(),
                timeline.map_or(0.0, |(_, _, alpha, _)| alpha),
                f32::from(u8::from(timeline.is_some() && !materialized)),
                0.0,
            ],
        }),
    );
    Ok(())
}
