//! Cohesive inputs for paged relation synchronization.

use crate::engine::chunk_draw_plan::ResidentRelationChunkPlacement;
use crate::scene_gpu::generic_visual::GenericVisualResources;
use crate::scene_gpu::instance_batch_table::GpuInstanceBatches;
use crate::scene_gpu::picking_pages::PickPages;
use molgfx_gpu::Device;

pub(in crate::scene_gpu) struct PagedRelationSync<'a, D: Device, F, A> {
    pub(in crate::scene_gpu) device: &'a D,
    pub(in crate::scene_gpu) queue: &'a D::Queue,
    pub(in crate::scene_gpu) render_layout: &'a D::BindGroupLayout,
    pub(in crate::scene_gpu) cull_layout: &'a D::BindGroupLayout,
    pub(in crate::scene_gpu) resolve_layout: &'a D::BindGroupLayout,
    pub(in crate::scene_gpu) display_source: &'a D::Buffer,
    pub(in crate::scene_gpu) generic_source: &'a D::Buffer,
    pub(in crate::scene_gpu) instance_sources: &'a GpuInstanceBatches<D>,
    pub(in crate::scene_gpu) plans: &'a [ResidentRelationChunkPlacement],
    pub(in crate::scene_gpu) instance_plans:
        &'a [crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
    pub(in crate::scene_gpu) picking: &'a PickPages,
    pub(in crate::scene_gpu) resources: GenericVisualResources<'a, D>,
    pub(in crate::scene_gpu) revision: u64,
    pub(in crate::scene_gpu) source_revision: u64,
    pub(in crate::scene_gpu) instance_binding_revision: u64,
    pub(in crate::scene_gpu) instance_timeline_revision: u64,
    pub(in crate::scene_gpu) attribute_timeline_revision: u64,
    pub(in crate::scene_gpu) visual_revision: u64,
    pub(in crate::scene_gpu) resolve: F,
    pub(in crate::scene_gpu) resolve_attribute: A,
}

pub(super) struct PagedGeometryInput<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) render_layout: &'a D::BindGroupLayout,
    pub(super) resolve_layout: &'a D::BindGroupLayout,
    pub(super) display_source: &'a D::Buffer,
    pub(super) generic_source: &'a D::Buffer,
    pub(super) instance_sources: &'a GpuInstanceBatches<D>,
    pub(super) plans: &'a [ResidentRelationChunkPlacement],
    pub(super) picking: &'a PickPages,
}
