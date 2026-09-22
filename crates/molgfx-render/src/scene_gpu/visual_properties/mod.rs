//! Scene-wide `array<u32>` arena for typed columns referenced by visual programs.
//!
//! A property is uploaded once even when several representations consume it.
//! The table is rebuilt only when visual bindings change; content revisions
//! rewrite the existing column directly from the caller-owned slice without a
//! second host-side copy.

mod binding;
mod plan;
mod timeline;
mod upload;

use super::grow_buffer::GrowBuffer;
use crate::engine::DerivedCacheKey;
use molgfx_core::VisualAttributeRef;
use molgfx_gpu::Device;

pub(super) const MISSING_CHUNK: [u32; 256] = [f32::NAN.to_bits(); 256];
pub(super) const MISSING_CHUNK_LEN: u32 = 256;

#[derive(Clone, Copy, Debug)]
pub(super) struct PropertyColumn {
    reference: VisualAttributeRef,
    offset: u32,
    length: u32,
    stride_words: u32,
    revision: u64,
    timeline_revision: u64,
    temporal: bool,
    storage_words: u32,
    materialized_offset: Option<u32>,
    cache_key: Option<DerivedCacheKey>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct AttributeTimelineConfig {
    offsets: [u32; 4],
    counts: [u32; 4],
}

#[derive(Debug)]
pub(super) struct AttributeTimelineGpu<D: Device> {
    config: D::Buffer,
    group: D::BindGroup,
    groups: [u32; 2],
    paged_plan: Option<crate::engine::chunk_draw_plan::ResidentAttributeMaterialization>,
}

#[derive(Clone, Copy)]
pub(crate) struct AttributeTimelineDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) groups: [u32; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::scene_gpu) struct AttributeArenaBinding {
    pub(in crate::scene_gpu) offsets: [u32; 4],
    pub(in crate::scene_gpu) layouts: [u32; 4],
}

/// One persistent, deduplicated column-major property table per scene.
#[derive(Debug)]
pub(in crate::scene_gpu) struct VisualPropertyTable<D: Device> {
    buffer: GrowBuffer<D>,
    columns: Vec<PropertyColumn>,
    planned_handles: Vec<VisualAttributeRef>,
    handle_scratch: Vec<VisualAttributeRef>,
    source_key: Option<(u64, u64, u64, u64)>,
    binding_revision: u64,
    timelines: Vec<AttributeTimelineGpu<D>>,
    paged_timelines: Vec<AttributeTimelineGpu<D>>,
    paged_source_revision: u64,
}

impl<D: Device> VisualPropertyTable<D> {
    pub(in crate::scene_gpu) fn new() -> Self {
        Self {
            buffer: GrowBuffer::new(),
            columns: Vec::new(),
            planned_handles: Vec::new(),
            handle_scratch: Vec::new(),
            source_key: None,
            binding_revision: 0,
            timelines: Vec::new(),
            paged_timelines: Vec::new(),
            paged_source_revision: 0,
        }
    }

    pub(in crate::scene_gpu) fn buffer(&self) -> Option<&D::Buffer> {
        self.buffer.get()
    }

    pub(in crate::scene_gpu) const fn binding_revision(&self) -> u64 {
        self.binding_revision
    }
}
