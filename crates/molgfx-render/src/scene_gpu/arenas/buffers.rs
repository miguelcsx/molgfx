//! Grow-only buffer helpers shared by scene slots.

use crate::error::RenderError;
use crate::graph::SHADOW_MAP_EXTENT;
use molgfx_core::DrawIndirectArgs;
use molgfx_gpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferDesc, BufferUsage, Device, Queue,
    ShaderStages,
};

#[cfg(test)]
#[path = "buffers_tests.rs"]
mod tests;

pub(in crate::scene_gpu) fn create_cull_tiles<D: Device>(
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

/// Bytes of the screen-depth buffer the cull passes share.
///
/// Tile LOD needs one word per 8x8 tile. Occlusion culling adds a depth plane
/// of one word per camera pixel and one per shadow texel; it is sized only for
/// scenes big enough to use it, and capped at the storage binding every
/// backend guarantees. A buffer under the cap is too small for the shader's own
/// check, which then draws every atom rather than cull on a partial plane.
pub(in crate::scene_gpu) fn cull_tiles_bytes(extent: [u32; 2], tiles: u32, occlusion: bool) -> u64 {
    const WORD: u64 = std::mem::size_of::<u32>() as u64;
    const BINDING_CAP: u64 = 128 << 20;
    let tile_bytes = u64::from(tiles) * WORD;
    let bytes = if occlusion {
        let shadow = u64::from(SHADOW_MAP_EXTENT) * u64::from(SHADOW_MAP_EXTENT);
        let camera = u64::from(extent[0]) * u64::from(extent[1]);
        tile_bytes.max((camera + shadow) * WORD).min(BINDING_CAP)
    } else {
        tile_bytes
    };
    bytes.max(256)
}

pub(in crate::scene_gpu) fn upload_grow<D: Device, T: bytemuck::Pod>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    records: &[T],
    buffer: &mut Option<D::Buffer>,
    capacity: &mut u64,
) -> Result<(), RenderError> {
    let bytes = bytemuck::cast_slice(records);
    let needed = bytes.len() as u64;
    ensure_upload_buffer(device, label, needed, buffer, capacity)?;
    if let Some(buffer) = buffer {
        queue.write_buffer(buffer, 0, bytes);
    }
    Ok(())
}

/// One slice of the scene-wide argument arena, as a bindable range.
///
/// The cull shader writes instance counts at "offset zero" of its own view, so
/// a slot binds a range starting at its own slot rather than the whole arena.
pub(in crate::scene_gpu) fn arena_range<D: Device>(
    binding: u32,
    arena: &D::Buffer,
    offset: u64,
) -> molgfx_gpu::BindGroupEntry<'_, D> {
    molgfx_gpu::BindGroupEntry::BufferRange {
        binding,
        buffer: arena,
        offset,
        size: super::indirect_arena::SLOT_STRIDE,
    }
}

/// Ensures a writable storage buffer without requiring a full staging slice.
///
/// This is the streaming counterpart to [`upload_grow`]: callers can reserve
/// the final table once and fill it through bounded chunks.
pub(in crate::scene_gpu) fn ensure_upload_buffer<D: Device>(
    device: &D,
    label: &'static str,
    needed: u64,
    buffer: &mut Option<D::Buffer>,
    capacity: &mut u64,
) -> Result<bool, RenderError> {
    if buffer.is_some() && needed <= *capacity {
        return Ok(false);
    }
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
    Ok(true)
}

fn grow_capacity(needed: u64, limit: u64, label: &'static str) -> Result<u64, RenderError> {
    if needed > limit || limit == 0 {
        return Err(limit_exceeded(label, limit));
    }
    let grown = match needed.checked_next_power_of_two() {
        Some(value) => value,
        None => needed,
    };
    Ok(grown.max(256).min(limit))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::scene_gpu) struct CullCounts {
    pub(in crate::scene_gpu) atoms: u32,
    pub(in crate::scene_gpu) bonds: u32,
    pub(in crate::scene_gpu) lod_enabled: u32,
    pub(in crate::scene_gpu) padding: u32,
    /// Model-space length past which a stretched bond is culled; zero disables.
    pub(in crate::scene_gpu) bond_break_length: f32,
    /// One when a typed visual result table should affect culling/shading.
    pub(in crate::scene_gpu) visual_enabled: u32,
    pub(in crate::scene_gpu) atom_bvh_nodes: u32,
    pub(in crate::scene_gpu) atom_bvh_indices: u32,
    pub(in crate::scene_gpu) quality_bvh_nodes: u32,
    pub(in crate::scene_gpu) quality_bvh_indices: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::scene_gpu) struct CullCountInput {
    pub(in crate::scene_gpu) atoms: u32,
    pub(in crate::scene_gpu) bonds: u32,
    pub(in crate::scene_gpu) lod_mode: u32,
    pub(in crate::scene_gpu) bond_break_length: f32,
    pub(in crate::scene_gpu) visual_enabled: bool,
    pub(in crate::scene_gpu) atom_bvh_nodes: u32,
    pub(in crate::scene_gpu) atom_bvh_indices: u32,
    pub(in crate::scene_gpu) quality_bvh_nodes: u32,
    pub(in crate::scene_gpu) quality_bvh_indices: u32,
}

impl From<CullCountInput> for CullCounts {
    fn from(input: CullCountInput) -> Self {
        Self {
            atoms: input.atoms,
            bonds: input.bonds,
            lod_enabled: input.lod_mode,
            padding: if input.lod_mode == 2 {
                input.atoms.div_ceil(65_536).max(1)
            } else {
                1
            },
            bond_break_length: input.bond_break_length,
            visual_enabled: u32::from(input.visual_enabled),
            atom_bvh_nodes: input.atom_bvh_nodes,
            atom_bvh_indices: input.atom_bvh_indices,
            quality_bvh_nodes: input.quality_bvh_nodes,
            quality_bvh_indices: input.quality_bvh_indices,
        }
    }
}

/// Writes cull counts into an already-allocated buffer.
pub(in crate::scene_gpu) fn write_counts_into<D: Device>(
    queue: &D::Queue,
    buffer: &D::Buffer,
    input: CullCountInput,
) {
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(&CullCounts::from(input)));
}

pub(in crate::scene_gpu) fn write_draw_args<D: Device>(
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

pub(in crate::scene_gpu) fn ensure_indices<D: Device>(
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

pub(in crate::scene_gpu) fn buffer_entry<D: Device>(
    binding: u32,
    buffer: &D::Buffer,
) -> BindGroupEntry<'_, D> {
    BindGroupEntry::Buffer { binding, buffer }
}

pub(in crate::scene_gpu) fn gpu_limit(resource: &'static str, limit: u64) -> molgfx_gpu::GpuError {
    molgfx_gpu::GpuError::LimitExceeded { resource, limit }
}

pub(in crate::scene_gpu) fn limit_exceeded(resource: &'static str, limit: u64) -> RenderError {
    gpu_limit(resource, limit).into()
}

pub(in crate::scene_gpu) fn create_buffer<D: Device>(
    device: &D,
    label: &'static str,
    size: u64,
    usage: BufferUsage,
) -> Result<D::Buffer, RenderError> {
    Ok(device.create_buffer(&BufferDesc { label, size, usage })?)
}

pub(in crate::scene_gpu) const fn storage_entry(
    binding: u32,
    read_only: bool,
    visibility: ShaderStages,
) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Storage { read_only },
    }
}

pub(in crate::scene_gpu) fn count(len: usize) -> u32 {
    crate::fallback(u32::try_from(len), u32::MAX)
}
