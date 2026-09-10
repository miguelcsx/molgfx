//! Stable identities, bounded conversions and buffer upload helpers.

use crate::error::RenderError;
use crate::scene_gpu::picking_pages::PickPages;
use pdviewx_core::{ChunkId, DatasetId, EntityKind, InstanceBatch, InstanceBatchHandle};
use pdviewx_gpu::{BufferDesc, BufferUsage, Device, Queue as _};
use pdviewx_math::Rgba8;
use std::sync::Arc;

pub(super) fn frame_identity(
    start: &Arc<[pdviewx_core::RigidInstance]>,
    end: &Arc<[pdviewx_core::RigidInstance]>,
) -> (usize, usize) {
    (
        Arc::as_ptr(start).cast::<()>() as usize,
        Arc::as_ptr(end).cast::<()>() as usize,
    )
}

pub(super) fn storage_buffer<D: Device, T: bytemuck::Pod>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    values: &[T],
) -> Result<D::Buffer, RenderError> {
    let bytes = std::mem::size_of_val(values) as u64;
    let buffer = device.create_buffer(&BufferDesc {
        label,
        size: bytes.max(std::mem::size_of::<T>() as u64).max(16),
        usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
    })?;
    if !values.is_empty() {
        queue.write_buffer(&buffer, 0, bytemuck::cast_slice(values));
    }
    Ok(buffer)
}

pub(super) fn template_part_page(
    picking: &PickPages,
    handle: InstanceBatchHandle,
    batch: &InstanceBatch,
) -> Result<u32, RenderError> {
    picking
        .page_for_table(
            DatasetId::new(batch.template().source_rows().namespace().0),
            table_chunk(handle),
            EntityKind::TemplatePart,
        )
        .ok_or(RenderError::PickingOwnerMissing)
}

fn table_chunk(handle: InstanceBatchHandle) -> ChunkId {
    ChunkId::new(u64::from(handle.row()) | (u64::from(handle.generation()) << 32))
}

pub(super) fn checked_count(count: usize) -> Result<u32, RenderError> {
    u32::try_from(count).map_err(|_| limit())
}

pub(super) const fn pack_color(color: Rgba8) -> u32 {
    color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16) | ((color.a as u32) << 24)
}

pub(super) fn limit() -> RenderError {
    pdviewx_gpu::GpuError::LimitExceeded {
        resource: "generic analytic instances",
        limit: u64::from(u32::MAX),
    }
    .into()
}
