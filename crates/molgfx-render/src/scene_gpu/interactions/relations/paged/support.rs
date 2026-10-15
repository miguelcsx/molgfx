//! Homogeneous paged-anchor binding and resolver helpers.

use super::{PagedAnchorKey, PagedRelationSources, PagedRigidConfig, PagedStreamKey};
use crate::engine::chunk_draw_plan::{ResidentPositionSource, ResidentSpatialAnchor};
use crate::error::RenderError;
use crate::scene_gpu::interaction_table::PagedRigidTimeline;
use crate::scene_gpu::uniforms::ModelUniforms;
use molgfx_gpu::{BindGroupEntry, BufferDesc, BufferUsage, Device, Queue as _};
use molgfx_math::Mat4;

pub(super) fn write_matching_rigid_alpha<D: Device>(
    queue: &D::Queue,
    source: Option<PagedRigidTimeline>,
    config: Option<&D::Buffer>,
    plans: &[crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
) {
    let Some(source) = source else { return };
    let alpha = plans.iter().find_map(|plan| {
        let timeline = plan.timeline?;
        (timeline.start_byte_offset == source.start_byte_offset
            && timeline.end_byte_offset == source.end_byte_offset)
            .then_some(timeline.interpolation)
    });
    if let (Some(config), Some(alpha)) = (config, alpha) {
        queue.write_buffer(config, 36, bytemuck::bytes_of(&alpha));
    }
}

pub(super) const fn rigid_timeline(key: PagedAnchorKey) -> Option<PagedRigidTimeline> {
    match key {
        PagedAnchorKey::Rigid {
            start_byte_offset,
            end_byte_offset,
            ..
        } => Some(PagedRigidTimeline {
            start_byte_offset,
            end_byte_offset,
        }),
        _ => None,
    }
}

pub(super) fn anchor_key(anchor: ResidentSpatialAnchor) -> PagedAnchorKey {
    match anchor {
        ResidentSpatialAnchor::World(_) => PagedAnchorKey::World,
        ResidentSpatialAnchor::Position {
            source,
            byte_offset,
            byte_len,
            model_to_world,
            ..
        } => PagedAnchorKey::Position {
            source: match source {
                ResidentPositionSource::Display => 0,
                ResidentPositionSource::Generic => 1,
            },
            byte_offset,
            byte_len,
            model: model_to_world.to_cols_array().map(f32::to_bits),
        },
        ResidentSpatialAnchor::Rigid {
            timeline_start,
            timeline_end,
            byte_len,
            interpolation,
            ..
        } => PagedAnchorKey::Rigid {
            start_byte_offset: timeline_start,
            end_byte_offset: timeline_end,
            byte_len,
            interpolation: interpolation.to_bits(),
        },
    }
}

pub(super) fn paged_anchor_payload(anchor: ResidentSpatialAnchor) -> [f32; 4] {
    match anchor {
        ResidentSpatialAnchor::World(position) => [position.x, position.y, position.z, 0.0],
        ResidentSpatialAnchor::Position { local_row, .. } => {
            [0.0, 0.0, 0.0, f32::from_bits(local_row)]
        }
        ResidentSpatialAnchor::Rigid {
            local_row,
            local_position,
            ..
        } => [
            local_position.x,
            local_position.y,
            local_position.z,
            f32::from_bits(local_row),
        ],
    }
}

pub(super) fn source_entry<'a, D: Device>(
    key: PagedAnchorKey,
    binding: u32,
    secondary: bool,
    sources: PagedRelationSources<'a, D>,
    fallback: &'a D::Buffer,
) -> BindGroupEntry<'a, D> {
    match key {
        PagedAnchorKey::World => BindGroupEntry::Buffer {
            binding,
            buffer: fallback,
        },
        PagedAnchorKey::Position {
            source,
            byte_offset,
            byte_len,
            ..
        } => BindGroupEntry::BufferRange {
            binding,
            buffer: if source == 0 {
                sources.display
            } else {
                sources.generic
            },
            offset: byte_offset,
            size: byte_len,
        },
        PagedAnchorKey::Rigid {
            start_byte_offset,
            end_byte_offset,
            byte_len,
            ..
        } => sources
            .instances
            .paged_materialized_source(start_byte_offset, end_byte_offset)
            .map_or(
                BindGroupEntry::BufferRange {
                    binding,
                    buffer: sources.generic,
                    offset: if secondary {
                        end_byte_offset
                    } else {
                        start_byte_offset
                    },
                    size: byte_len,
                },
                |buffer| BindGroupEntry::Buffer { binding, buffer },
            ),
    }
}

pub(super) fn create_rigid_config<D: Device>(
    device: &D,
    queue: &D::Queue,
    key: PagedAnchorKey,
) -> Result<Option<D::Buffer>, RenderError> {
    let PagedAnchorKey::Rigid { interpolation, .. } = key else {
        return Ok(None);
    };
    let buffer = device.create_buffer(&BufferDesc {
        label: "paged relation rigid timeline",
        size: std::mem::size_of::<PagedRigidConfig>() as u64,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    queue.write_buffer(
        &buffer,
        0,
        bytemuck::bytes_of(&PagedRigidConfig {
            counts: [0; 4],
            picking_style: [0; 4],
            bounds: [0.0, f32::from_bits(interpolation), 1.0, 0.0],
        }),
    );
    Ok(Some(buffer))
}

pub(super) fn create_model<D: Device>(
    device: &D,
    queue: &D::Queue,
    key: PagedAnchorKey,
) -> Result<Option<D::Buffer>, RenderError> {
    let PagedAnchorKey::Position { model, .. } = key else {
        return Ok(None);
    };
    let model = Mat4::from_cols_array(&model.map(f32::from_bits));
    let buffer = device.create_buffer(&BufferDesc {
        label: "paged relation source model",
        size: std::mem::size_of::<ModelUniforms>() as u64,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    queue.write_buffer(
        &buffer,
        0,
        bytemuck::bytes_of(&ModelUniforms::new(model, model, [u32::MAX; 9])),
    );
    Ok(Some(buffer))
}

pub(super) fn paged_pipeline(key: PagedStreamKey) -> u8 {
    paged_kernel(key.start) * 4 + paged_kernel(key.end)
}

const fn paged_kernel(key: PagedAnchorKey) -> u8 {
    match key {
        PagedAnchorKey::World => 0,
        PagedAnchorKey::Position { .. } => 2,
        PagedAnchorKey::Rigid { .. } => 3,
    }
}

pub(super) const fn tracks_coordinates(key: PagedAnchorKey) -> bool {
    matches!(
        key,
        PagedAnchorKey::Position { source: 0, .. } | PagedAnchorKey::Rigid { .. }
    )
}

pub(super) fn missing_paged_source() -> RenderError {
    RenderError::Residency {
        reason: "paged relation source occurrence is not resident",
    }
}
