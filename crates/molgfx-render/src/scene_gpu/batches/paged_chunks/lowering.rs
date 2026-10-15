//! Validated lowering of paged placements into GPU records.

use super::{
    POINT_COMMAND, POINT_REPRESENTATION, PlacementGpu, SPACEFILL_COMMAND, SPACEFILL_REPRESENTATION,
    TrajectoryWindowGpu,
};
use crate::RenderError;
use crate::engine::chunk_draw_plan::{ResidentChunkPlacement, ResidentTrajectoryWindow};

pub(super) fn lower(
    value: &ResidentChunkPlacement,
    page: u32,
    dynamic_coordinates: bool,
) -> Result<PlacementGpu, RenderError> {
    let coordinate_base = local_offset(value.range.coordinate_allocation.byte_offset(), 4)?;
    let cluster_base = local_offset(
        value.range.cluster_allocation.byte_offset(),
        std::mem::size_of::<crate::engine::chunk_draw_plan::ChunkClusterGpu>() as u64,
    )?;
    let (representation, size, cluster_padding, color) = match value.representation() {
        crate::engine::ChunkRepresentation::Points {
            diameter_pixels,
            color,
        } => (POINT_REPRESENTATION, diameter_pixels, 0.0, color),
        crate::engine::ChunkRepresentation::Spacefill {
            radius_scale,
            color,
        } => {
            let cluster_padding = value.range.max_radius * radius_scale;
            if !cluster_padding.is_finite() {
                return Err(RenderError::Residency {
                    reason: "paged spacefill radius exceeds finite GPU geometry",
                });
            }
            (
                SPACEFILL_REPRESENTATION,
                radius_scale,
                cluster_padding,
                color,
            )
        }
    };
    Ok(PlacementGpu {
        model_to_world: value.model_to_world.to_cols_array(),
        coordinate_base,
        radius_base: value.range.radius_base,
        cluster_base,
        cluster_count: value.range.cluster_count,
        local_rows: value.range.span.row_count(),
        pick_page: page,
        color: u32::from_le_bytes([color.r, color.g, color.b, color.a]),
        representation,
        size,
        cluster_padding,
        dynamic_coordinates: u32::from(dynamic_coordinates),
        _padding: 0,
    })
}

pub(super) fn lower_window(
    value: &ResidentTrajectoryWindow,
) -> Result<TrajectoryWindowGpu, RenderError> {
    Ok(TrajectoryWindowGpu {
        output_base: local_offset(value.structure.coordinate_allocation.byte_offset(), 4)?,
        start_base: local_offset(value.start.byte_offset, 4)?,
        end_base: local_offset(value.end.byte_offset, 4)?,
        count: value.start.local_rows,
        interpolation: value.interpolation,
        _padding: [0; 3],
    })
}

pub(super) const fn representation_command(value: crate::engine::ChunkRepresentation) -> usize {
    match value {
        crate::engine::ChunkRepresentation::Points { .. } => POINT_COMMAND,
        crate::engine::ChunkRepresentation::Spacefill { .. } => SPACEFILL_COMMAND,
    }
}

pub(super) fn local_offset(bytes: u64, stride: u64) -> Result<u32, RenderError> {
    if !bytes.is_multiple_of(stride) {
        return Err(RenderError::Residency {
            reason: "paged chunk arena offset is misaligned",
        });
    }
    u32::try_from(bytes / stride).map_err(|_| RenderError::Residency {
        reason: "paged chunk arena offset exceeds local u32 addressing",
    })
}

pub(super) fn bytes_for<T>(count: usize) -> Result<u64, RenderError> {
    let bytes =
        std::mem::size_of::<T>()
            .checked_mul(count.max(1))
            .ok_or(RenderError::Residency {
                reason: "paged chunk buffer size overflow",
            })?;
    u64::try_from(bytes).map_err(|_| RenderError::Residency {
        reason: "paged chunk buffer exceeds the host address space",
    })
}
