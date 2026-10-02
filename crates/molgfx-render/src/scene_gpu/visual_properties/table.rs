//! The persistent, deduplicated property table of a scene.

use super::super::grow_buffer::GrowBuffer;
use super::{AttributeTimelineGpu, PropertyColumn, StateColumn};
use molgfx_core::VisualAttributeRef;
use molgfx_gpu::Device;

/// One persistent, deduplicated column-major property table per scene.
#[derive(Debug)]
pub(in crate::scene_gpu) struct VisualPropertyTable<D: Device> {
    pub(super) buffer: GrowBuffer<D>,
    pub(super) columns: Vec<PropertyColumn>,
    pub(super) states: Vec<StateColumn>,
    pub(super) planned_handles: Vec<VisualAttributeRef>,
    pub(super) handle_scratch: Vec<VisualAttributeRef>,
    pub(super) source_key: Option<(u64, u64, u64, u64, u64)>,
    pub(super) binding_revision: u64,
    pub(super) timelines: Vec<AttributeTimelineGpu<D>>,
    pub(super) paged_timelines: Vec<AttributeTimelineGpu<D>>,
    pub(super) paged_source_revision: u64,
}

impl<D: Device> VisualPropertyTable<D> {
    pub(in crate::scene_gpu) fn new() -> Self {
        Self {
            buffer: GrowBuffer::new(),
            columns: Vec::new(),
            states: Vec::new(),
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
