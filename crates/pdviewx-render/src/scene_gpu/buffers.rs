//! Grow-only buffer helpers shared by scene slots.

use super::slot_types::CullCounts;
use crate::error::RenderError;
use pdviewx_core::DrawIndirectArgs;
use pdviewx_gpu::{BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};

#[cfg(test)]
#[path = "buffers_tests.rs"]
mod tests;

pub(super) fn create_cull_tiles<D: Device>(
    device: &D,
    bytes: u64,
) -> Result<(D::Buffer, u64), RenderError> {
    let buffer = device.create_buffer(&BufferDesc {
        label: "screen tile visibility",
        size: bytes,
        usage: BufferUsage::STORAGE,
    })?;
    Ok((buffer, bytes))
}

pub(super) fn upload_grow<D: Device, T: bytemuck::Pod>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    records: &[T],
    buffer: &mut Option<D::Buffer>,
    capacity: &mut u64,
) -> Result<(), RenderError> {
    let bytes = bytemuck::cast_slice(records);
    let needed = bytes.len() as u64;
    if buffer.is_none() || needed > *capacity {
        *capacity = grow_capacity(
            needed,
            device.capabilities().max_storage_buffer_bytes,
            label,
        )?;
        *buffer = Some(device.create_buffer(&BufferDesc {
            label,
            size: *capacity,
            usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
        })?);
    }
    if let Some(buffer) = buffer {
        queue.write_buffer(buffer, 0, bytes);
    }
    Ok(())
}

fn grow_capacity(needed: u64, limit: u64, label: &'static str) -> Result<u64, RenderError> {
    if needed > limit || limit == 0 {
        return Err(pdviewx_gpu::GpuError::LimitExceeded {
            resource: label,
            limit,
        }
        .into());
    }
    let grown = match needed.checked_next_power_of_two() {
        Some(value) => value,
        None => needed,
    };
    Ok(grown.max(256).min(limit))
}

pub(super) fn write_args<D: Device>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    buffer: &mut Option<D::Buffer>,
) -> Result<(), RenderError> {
    write_draw_args(device, queue, label, 6, 0, buffer)
}

pub(super) fn write_counts<D: Device>(
    device: &D,
    queue: &D::Queue,
    atoms: u32,
    bonds: u32,
    lod_mode: u32,
    buffer: &mut Option<D::Buffer>,
) -> Result<(), RenderError> {
    if buffer.is_none() {
        *buffer = Some(device.create_buffer(&BufferDesc {
            label: "cull counts",
            size: std::mem::size_of::<CullCounts>() as u64,
            usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
        })?);
    }
    if let Some(buffer) = buffer {
        queue.write_buffer(
            buffer,
            0,
            bytemuck::bytes_of(&CullCounts {
                atoms,
                bonds,
                lod_enabled: lod_mode,
                padding: if lod_mode == 2 {
                    atoms.div_ceil(65_536).max(1)
                } else {
                    1
                },
            }),
        );
    }
    Ok(())
}

pub(super) fn write_draw_args<D: Device>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    vertex_count: u32,
    instance_count: u32,
    buffer: &mut Option<D::Buffer>,
) -> Result<(), RenderError> {
    if buffer.is_none() {
        *buffer = Some(
            device.create_buffer(&BufferDesc {
                label,
                size: std::mem::size_of::<DrawIndirectArgs>() as u64,
                usage: BufferUsage::INDIRECT
                    .union(BufferUsage::STORAGE)
                    .union(BufferUsage::COPY_DST),
            })?,
        );
    }
    if let Some(buffer) = buffer {
        let args = DrawIndirectArgs {
            vertex_count,
            instance_count,
            first_vertex: 0,
            first_instance: 0,
        };
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(&args));
    }
    Ok(())
}

pub(super) fn ensure_indices<D: Device>(
    device: &D,
    label: &'static str,
    count: u32,
    buffer: &mut Option<D::Buffer>,
    capacity: &mut u64,
) -> Result<(), RenderError> {
    let needed = u64::from(count) * std::mem::size_of::<u32>() as u64;
    if buffer.is_none() || needed > *capacity {
        *capacity = needed.next_power_of_two().max(256);
        *buffer = Some(device.create_buffer(&BufferDesc {
            label,
            size: *capacity,
            usage: BufferUsage::STORAGE,
        })?);
    }
    Ok(())
}

pub(super) fn buffer_entry<D: Device>(binding: u32, buffer: &D::Buffer) -> BindGroupEntry<'_, D> {
    BindGroupEntry::Buffer { binding, buffer }
}

pub(super) fn count(len: usize) -> u32 {
    u32::try_from(len).map_or(u32::MAX, |value| value)
}
