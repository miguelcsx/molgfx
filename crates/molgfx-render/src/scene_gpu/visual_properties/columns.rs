//! The column records of the property arena.

use crate::engine::DerivedCacheKey;
use molgfx_core::{StructureHandle, VisualAttributeRef};
use molgfx_gpu::Device;

pub(super) const MISSING_CHUNK: [u32; 256] = [f32::NAN.to_bits(); 256];
pub(super) const MISSING_CHUNK_LEN: u32 = 256;

#[derive(Clone, Copy, Debug)]
pub(super) struct PropertyColumn {
    pub(super) reference: VisualAttributeRef,
    pub(super) offset: u32,
    pub(super) length: u32,
    pub(super) stride_words: u32,
    pub(super) revision: u64,
    pub(super) timeline_revision: u64,
    pub(super) temporal: bool,
    pub(super) storage_words: u32,
    pub(super) materialized_offset: Option<u32>,
    pub(super) cache_key: Option<DerivedCacheKey>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct StateColumn {
    pub(super) structure: StructureHandle,
    pub(super) offset: u32,
    pub(super) length: u32,
    pub(super) revision: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct AttributeTimelineConfig {
    pub(super) offsets: [u32; 4],
    pub(super) counts: [u32; 4],
}

#[derive(Debug)]
pub(super) struct AttributeTimelineGpu<D: Device> {
    pub(super) config: D::Buffer,
    pub(super) group: D::BindGroup,
    pub(super) groups: [u32; 2],
    pub(super) paged_plan: Option<crate::engine::chunk_draw_plan::ResidentAttributeMaterialization>,
}
