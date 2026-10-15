//! Stable relation keys and limits.

use super::{
    ChunkId, Device, GpuStructure, RelationResolverGpu, RenderError, RowDomain, Scene,
    anchor_payload, gpu_limit,
};

pub(super) fn relation_resolver(
    scene: &Scene,
    relation: molgfx_core::Relation,
    batch: &molgfx_core::RelationBatch,
    logical: u32,
    output_row: u32,
) -> Result<RelationResolverGpu, RenderError> {
    let angle = match batch
        .oriented_geometry()
        .and_then(|rows| rows.get(logical as usize))
    {
        Some(constraint) => constraint
            .geometry
            .minimum_angle_degrees
            .to_radians()
            .to_bits(),
        None => 0,
    };
    Ok(RelationResolverGpu {
        start: anchor_payload(scene, relation.start)?,
        end: anchor_payload(scene, relation.end)?,
        output: [
            output_row,
            logical,
            u32::from(batch.oriented_geometry().is_some()),
            angle,
        ],
        reserved: oriented_geometry_payload(batch, logical),
    })
}

fn oriented_geometry_payload(batch: &molgfx_core::RelationBatch, logical: u32) -> [u32; 4] {
    let Some(constraint) = batch
        .oriented_geometry()
        .and_then(|rows| rows.get(logical as usize))
    else {
        return [0; 4];
    };
    let pivot_distance_squared = match constraint.geometry.maximum_pivot_end_distance {
        Some(distance) => distance * distance,
        None => f32::INFINITY,
    };
    [
        constraint.pivot.row(),
        (constraint.geometry.maximum_endpoint_distance
            * constraint.geometry.maximum_endpoint_distance)
            .to_bits(),
        constraint
            .geometry
            .minimum_angle_degrees
            .to_radians()
            .cos()
            .to_bits(),
        pivot_distance_squared.to_bits(),
    ]
}

pub(super) fn table_chunk(row: u32, generation: u32) -> ChunkId {
    ChunkId::new(u64::from(row) | (u64::from(generation) << 32))
}

// O(rows) scratch retains logical order without depending on atomic retirement.
pub(super) fn stable_relation_scratch_bytes(rows: u32) -> u64 {
    u64::from(rows) * 8 + u64::from(rows).div_ceil(64) * 4
}

pub(super) fn source_binding_revision<D: Device>(structures: &[GpuStructure<D>]) -> u64 {
    structures
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, source| {
            hash.wrapping_mul(0x0000_0100_0000_01b3)
                ^ u64::from(source.handle.row())
                ^ (u64::from(source.handle.generation()) << 32)
                ^ source.binding_revision
        })
}

pub(super) fn row_limit() -> molgfx_gpu::GpuError {
    gpu_limit("generic relation rows", u64::from(u32::MAX))
}

pub(super) fn missing_domain(domain: Option<RowDomain>) -> RenderError {
    let Some(domain) = domain else {
        return RenderError::PickingOwnerMissing;
    };
    RenderError::RelationSourceMissing { domain }
}
