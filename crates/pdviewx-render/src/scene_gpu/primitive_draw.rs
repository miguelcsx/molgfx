//! Shape-grouped draw ranges over the packed primitive table.
//!
//! Each analytic primitive family — and each particle shape within the
//! particle family — is intersected by its own fragment pipeline, so a draw
//! must contain only one family (and, for particles, one shape): feeding a box
//! record to the ellipsoid intersection would paint the wrong surface. The
//! records are packed into one buffer, sorted here so every class is
//! contiguous, and then each class is drawn as a range starting at its first
//! instance. The sort key is stable, so the order — and therefore the frame —
//! is reproducible.

use pdviewx_core::{ParticleMotionGpu, PrimitiveGpu};

#[cfg(test)]
#[path = "primitive_draw_tests.rs"]
mod tests;

/// The primitive family carried in `PrimitiveGpu.metadata[2]`.
///
/// These match the values the packing writes and the constants the shader
/// tests; ellipsoid, polygon and box are intersected analytically, while the
/// particle family is specialized further by shape.
pub(crate) const FAMILY_ELLIPSOID: u32 = 0;
pub(crate) const FAMILY_POLYGON: u32 = 1;
pub(crate) const FAMILY_BOX: u32 = 2;
pub(crate) const FAMILY_PARTICLE: u32 = 3;
pub(crate) const POLYGON_PENTAGON: u32 = 5;
pub(crate) const POLYGON_HEXAGON: u32 = 6;

/// Distinct particle shapes, so a caller can size a per-shape pipeline table.
pub(crate) const PARTICLE_SHAPES: u32 = 8;

/// One packed record with the auxiliary columns that must move with it.
///
/// The previous centre and the motion sample are indexed in lockstep with the
/// record by the advection compute pass, so sorting the record without them
/// would desynchronize motion. They travel together and are split back out
/// into the upload columns only after the sort.
#[derive(Debug)]
pub(super) struct PackedPrimitive {
    pub(super) record: PrimitiveGpu,
    pub(super) previous: [f32; 4],
    pub(super) motion: ParticleMotionGpu,
    sort_key: u32,
}

impl PackedPrimitive {
    pub(super) fn new(mut record: PrimitiveGpu, motion: ParticleMotionGpu) -> Self {
        let previous = record.center_radius;
        record.inverse_cross[3] = if motion.metadata[0] == 0 { 0.0 } else { 1.0 };
        let sort_key = sort_key(&record);
        Self {
            record,
            previous,
            motion,
            sort_key,
        }
    }
}

/// A contiguous run of one class in the sorted table.
///
/// `first` and `len` bound a direct instanced draw; the vertex stage reads
/// `primitive[instance_index]`, and an instance index already includes the
/// first-instance offset, so the range needs no buffer rebinding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PrimitiveDrawGroup {
    /// The primitive family this run draws.
    pub(crate) family: u32,
    /// Particle shape or polygon side class; zero for the other families.
    pub(crate) shape: u32,
    /// Whether the run is order-independent-transparent geometry.
    pub(crate) translucent: bool,
    /// First instance index into the sorted table.
    pub(crate) first: u32,
    /// Number of instances in the run.
    pub(crate) len: u32,
}

/// The opaque family field, or the particle shape, folded into one small code
/// so records of one class sort together. Opaque and translucent runs are kept
/// apart by the high bit, since they draw in different passes.
fn sort_key(record: &PrimitiveGpu) -> u32 {
    let family = record.metadata[2];
    let shape = draw_shape(family, record.metadata[3]);
    let class = family * PARTICLE_SHAPES + shape;
    let translucent = u32::from(record.color[3] < 0.999);
    (translucent << 16) | class
}

/// Sorts `rows` into class order and rewrites the upload columns and the draw
/// groups from it. Auxiliary columns are materialized only when at least one
/// primitive moves; a fully static table binds one dummy row instead. The sort
/// is stable, so equal keys keep insertion order and deterministic frames. All
/// outputs retain their capacity for reuse.
pub(super) fn regroup(
    rows: &mut [PackedPrimitive],
    records: &mut Vec<PrimitiveGpu>,
    previous: &mut Vec<[f32; 4]>,
    motion: &mut Vec<ParticleMotionGpu>,
    groups: &mut Vec<PrimitiveDrawGroup>,
    include_auxiliary: bool,
) {
    if rows
        .windows(2)
        .any(|pair| pair[0].sort_key > pair[1].sort_key)
    {
        rows.sort_by_key(|row| row.sort_key);
    }
    records.clear();
    previous.clear();
    motion.clear();
    groups.clear();
    for row in rows.iter() {
        let first = u32::try_from(records.len()).map_or(u32::MAX, |value| value);
        records.push(row.record);
        if include_auxiliary {
            previous.push(row.previous);
            motion.push(row.motion);
        }
        let family = row.record.metadata[2];
        let shape = draw_shape(family, row.record.metadata[3]);
        let translucent = row.record.color[3] < 0.999;
        match groups.last_mut() {
            Some(group)
                if group.family == family
                    && group.translucent == translucent
                    && group.shape == shape =>
            {
                group.len += 1;
            }
            _ => groups.push(PrimitiveDrawGroup {
                family,
                shape,
                translucent,
                first,
                len: 1,
            }),
        }
    }
}

const fn draw_shape(family: u32, shape: u32) -> u32 {
    match family {
        FAMILY_PARTICLE => {
            if shape < PARTICLE_SHAPES {
                shape
            } else {
                PARTICLE_SHAPES - 1
            }
        }
        FAMILY_POLYGON if shape == 4 || shape == 5 => POLYGON_PENTAGON,
        FAMILY_POLYGON => POLYGON_HEXAGON,
        _ => 0,
    }
}
