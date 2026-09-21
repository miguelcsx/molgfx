//! Hierarchy construction: spatial keys, splits and leaf aggregation.
//!
//! Split out from the hierarchy itself because building and querying are two
//! responsibilities with almost no shared state — the query path touches
//! finished nodes, this path touches source geometry.
//!
//! The two `O(primitives)` phases here run across cores above a threshold. Both
//! are exact and order-independent: the scene bound is a componentwise minimum
//! and maximum, and the key vector is collected in source order regardless of
//! which thread produced which entry. Neither can give a different answer for a
//! different core count, which is what lets a golden image survive the change.

use super::BvhBuildError;
use super::bvh::{
    Bvh, BvhNode, INDEX_MASK, LEAF_SIZE, MAX_BRANCH_LEFT, MORTON_LEVELS, MortonEntry,
};
use super::source::BvhSource;
use crate::{Aabb, Vec3};
use rayon::prelude::*;

/// Primitive count above which key generation partitions across cores.
const PARALLEL_KEY_THRESHOLD: usize = 1 << 16;

/// Tightest bound over every valid primitive the source supplies.
pub(super) fn scene_bounds<S: BvhSource + ?Sized>(source: &S, count: u32) -> Aabb {
    let fold = |bounds: Aabb, index: u32| {
        let bound = source.bound(index);
        if valid_bound(&bound) {
            bounds.union(&bound)
        } else {
            bounds
        }
    };
    if (count as usize) < PARALLEL_KEY_THRESHOLD {
        return (0..count).fold(Aabb::EMPTY, fold);
    }
    (0..count)
        .into_par_iter()
        .fold(|| Aabb::EMPTY, fold)
        .reduce(|| Aabb::EMPTY, |left, right| left.union(&right))
}

/// Appends one key per valid primitive, in ascending source order.
///
/// Source order is what lets the sort stay keyed on the code alone: the sort is
/// stable, so equal codes come out in the order they went in. `ParallelExtend`
/// preserves the iterator's order, so the parallel path produces the same
/// vector as the serial one.
pub(super) fn extend_keys<S: BvhSource + ?Sized>(
    entries: &mut Vec<MortonEntry>,
    source: &S,
    count: u32,
    scene_bounds: Aabb,
) {
    let key = |index: u32| {
        let bound = source.bound(index);
        valid_bound(&bound).then(|| MortonEntry {
            code: morton_code(bound.center(), scene_bounds),
            source: index,
        })
    };
    if (count as usize) < PARALLEL_KEY_THRESHOLD {
        entries.extend((0..count).filter_map(key));
        return;
    }
    entries.par_extend((0..count).into_par_iter().filter_map(key));
}

pub(super) fn build_range<S: BvhSource + ?Sized>(
    entries: &[MortonEntry],
    source: &S,
    start: usize,
    end: usize,
    node: usize,
    out: &mut Bvh,
) -> Result<BuildAggregate, BvhBuildError> {
    if end - start <= LEAF_SIZE {
        let first = checked_u32(
            "BVH leaf primitive offset",
            out.primitive_indices.len(),
            INDEX_MASK,
        )?;
        let mut bounds = Aabb::EMPTY;
        let mut max_radius = 0.0f32;
        for entry in &entries[start..end] {
            let bound = source.bound(entry.source);
            bounds = bounds.union(&bound);
            max_radius = max_radius.max(bound.half_extents().max_element());
            out.primitive_indices.push(entry.source);
        }
        let count = checked_u32("BVH leaf primitive count", end - start, 7)?;
        out.nodes[node] = BvhNode::leaf(bounds, first, count, max_radius);
        return Ok(BuildAggregate { bounds, max_radius });
    }

    let split = split_range(entries, start, end);
    let left = out.nodes.len();
    out.nodes.extend([BvhNode::default(), BvhNode::default()]);
    let left_bounds = build_range(entries, source, start, split, left, out)?;
    let right_bounds = build_range(entries, source, split, end, left + 1, out)?;
    let bounds = left_bounds.bounds.union(&right_bounds.bounds);
    let max_radius = left_bounds.max_radius.max(right_bounds.max_radius);
    let left = checked_u32("BVH left child", left, MAX_BRANCH_LEFT)?;
    out.nodes[node] = BvhNode::branch(bounds, left, max_radius);
    Ok(BuildAggregate { bounds, max_radius })
}

#[inline]
pub(super) fn checked_index(
    resource: &'static str,
    index: usize,
    maximum: u32,
) -> Result<(), BvhBuildError> {
    checked_u32(resource, index, maximum).map(|_| ())
}

#[inline]
pub(super) fn checked_u32(
    resource: &'static str,
    index: usize,
    maximum: u32,
) -> Result<u32, BvhBuildError> {
    let index_u64 = u64::try_from(index)
        .into_iter()
        .fold(u64::MAX, |_, value| value);
    let converted = u32::try_from(index).map_err(|_| BvhBuildError {
        resource,
        index: index_u64,
        maximum,
    })?;
    if converted > maximum {
        return Err(BvhBuildError {
            resource,
            index: index_u64,
            maximum,
        });
    }
    Ok(converted)
}

#[derive(Clone, Copy)]
pub(super) struct BuildAggregate {
    bounds: Aabb,
    max_radius: f32,
}

#[inline]
fn split_range(entries: &[MortonEntry], start: usize, end: usize) -> usize {
    let first = entries[start].code;
    let last = entries[end - 1].code;
    if first == last {
        return start + (end - start) / 2;
    }
    let shared_prefix = (first ^ last).leading_zeros();
    let mut low = start + 1;
    let mut high = end - 1;
    while low < high {
        let middle = (low + high).div_ceil(2);
        if (first ^ entries[middle].code).leading_zeros() > shared_prefix {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low + 1
}

#[inline]
pub(super) fn valid_bound(bound: &Aabb) -> bool {
    !bound.is_empty() && bound.min.is_finite() && bound.max.is_finite()
}

#[inline]
pub(super) fn morton_code(point: Vec3, bounds: Aabb) -> u64 {
    let extent = (bounds.max - bounds.min).max(Vec3::splat(f32::EPSILON));
    let unit = ((point - bounds.min) / extent).clamp(Vec3::ZERO, Vec3::ONE);
    let x = crate::unit_to_grid(unit.x, MORTON_LEVELS);
    let y = crate::unit_to_grid(unit.y, MORTON_LEVELS);
    let z = crate::unit_to_grid(unit.z, MORTON_LEVELS);
    expand_21(x) | (expand_21(y) << 1) | (expand_21(z) << 2)
}

#[inline]
/// Spreads 21 bits so each occupies every third position, leaving room for the
/// other two axes to interleave into the gaps.
pub(super) const fn expand_21(value: u32) -> u64 {
    let mut value = (value as u64) & 0x001f_ffff;
    value = (value | (value << 32)) & 0x001f_0000_0000_ffff;
    value = (value | (value << 16)) & 0x001f_0000_ff00_00ff;
    value = (value | (value << 8)) & 0x100f_00f0_0f00_f00f;
    value = (value | (value << 4)) & 0x10c3_0c30_c30c_30c3;
    value = (value | (value << 2)) & 0x1249_2492_4924_9249;
    value
}
