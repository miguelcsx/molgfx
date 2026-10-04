//! Native indexed draw resources shared by generated and caller meshes.

use crate::error::RenderError;
use molgfx_gpu::{BufferDesc, BufferUsage, Device, Queue};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct IndexedArguments {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

pub(super) struct IndexedDraw<'a, D: Device> {
    pub(super) group: &'a D::BindGroup,
    pub(super) arguments: &'a D::Buffer,
    pub(super) indices: &'a D::Buffer,
}

pub(super) fn write_arguments<D: Device>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    index_count: u32,
    buffer: &mut Option<D::Buffer>,
) -> Result<(), RenderError> {
    if buffer.is_none() {
        *buffer = Some(device.create_buffer(&BufferDesc {
            label,
            size: std::mem::size_of::<IndexedArguments>() as u64,
            usage: BufferUsage::INDIRECT.union(BufferUsage::COPY_DST),
        })?);
    }
    if let Some(buffer) = buffer {
        let arguments = IndexedArguments {
            index_count,
            instance_count: u32::from(index_count > 0),
            first_index: 0,
            base_vertex: 0,
            first_instance: 0,
        };
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(&arguments));
    }
    Ok(())
}

#[cfg(test)]
#[path = "indexed_draw_tests.rs"]
mod tests;
