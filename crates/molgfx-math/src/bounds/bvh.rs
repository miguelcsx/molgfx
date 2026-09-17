//! Deterministic linear bounding-volume hierarchy.
//!
//! Construction radix-sorts `n` primitive bounds by a 63-bit Morton key in
//! `O(n)` and emits `O(n)` compact nodes. Traversal is `O(log n + hits)` for
//! coherent spatial data and accepts caller-owned scratch vectors, so repeated
//! queries allocate nothing. The 32-byte node is directly uploadable to storage
//! buffers.
//!
//! The key is 21 bits per axis rather than ten. Ten bits addresses a 1024³
//! lattice, so past roughly ten million primitives a dense region collapses
//! into one cell, every key there is equal, and the split degenerates to a
//! median that ignores geometry — the hierarchy stops separating what it is
//! built to separate. Twenty-one bits addresses a 2,097,152³ lattice, which
//! keeps neighbouring primitives distinguishable at the scales this engine
//! claims.

use super::BvhBuildError;
use super::source::BvhSource;
use crate::{Aabb, Vec3};
use std::ops::Range;

use super::build;
use build::{checked_index, checked_u32};

#[cfg(test)]
#[path = "bvh_tests.rs"]
mod tests;

#[path = "bvh_query.rs"]
mod query;
#[cfg(test)]
use query::point_box_distance_squared;

pub(super) const LEAF_SIZE: usize = 4;

/// Cells per axis in the Morton lattice: 21 bits, minus one so the top index is
/// reachable from a unit coordinate of exactly one.
pub(super) const MORTON_LEVELS: u32 = (1 << 21) - 1;

/// Primitive count above which the key sort partitions across cores. Below it
/// the pool would cost more than the sort.
const PARALLEL_SORT_THRESHOLD: usize = 1 << 16;
pub(super) const COUNT_SHIFT: u32 = 29;
pub(super) const INDEX_MASK: u32 = (1 << COUNT_SHIFT) - 1;
pub(super) const MAX_BRANCH_LEFT: u32 = INDEX_MASK - 1;

/// One compact BVH node. Internal children are adjacent; leaves address the
/// hierarchy's primitive-index array.
#[repr(C, align(16))]
#[derive(Clone, Copy, PartialEq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BvhNode {
    /// Minimum corner followed by left-child or first-primitive index.
    pub min_left: [f32; 4],
    /// Maximum corner followed by the largest primitive half-extent.
    pub max_radius: [f32; 4],
}

impl BvhNode {
    // `build_range` validates both offsets before reaching this encoder.
    #[inline]
    pub(super) fn leaf(bounds: Aabb, first: u32, count: u32, max_radius: f32) -> Self {
        let metadata = first | (count << COUNT_SHIFT);
        Self {
            min_left: [
                bounds.min.x,
                bounds.min.y,
                bounds.min.z,
                f32::from_bits(metadata),
            ],
            max_radius: [bounds.max.x, bounds.max.y, bounds.max.z, max_radius],
        }
    }

    // `build_range` validates the child offset before reaching this encoder.
    #[inline]
    pub(super) fn branch(bounds: Aabb, left: u32, max_radius: f32) -> Self {
        Self {
            min_left: [
                bounds.min.x,
                bounds.min.y,
                bounds.min.z,
                f32::from_bits(left),
            ],
            max_radius: [bounds.max.x, bounds.max.y, bounds.max.z, max_radius],
        }
    }

    /// Node bounds.
    #[must_use]
    #[inline]
    pub fn bounds(self) -> Aabb {
        Aabb::new(
            Vec3::from_array([self.min_left[0], self.min_left[1], self.min_left[2]]),
            Vec3::from_array([self.max_radius[0], self.max_radius[1], self.max_radius[2]]),
        )
    }

    /// Largest primitive half-extent below this node.
    #[must_use]
    #[inline]
    pub fn maximum_radius(self) -> f32 {
        self.max_radius[3]
    }

    /// True for a leaf node.
    #[must_use]
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.min_left[3].to_bits() >> COUNT_SHIFT != 0
    }

    /// Adjacent child indices for an internal node.
    #[must_use]
    #[inline]
    pub fn children(self) -> Option<(u32, u32)> {
        if self.is_leaf() {
            return None;
        }
        let left = self.min_left[3].to_bits() & INDEX_MASK;
        Some((left, left + 1))
    }

    /// Range into `Bvh::primitive_indices` for a leaf.
    #[must_use]
    #[inline]
    pub fn primitive_range(self) -> Option<Range<u32>> {
        if !self.is_leaf() {
            return None;
        }
        let metadata = self.min_left[3].to_bits();
        let first = metadata & INDEX_MASK;
        let count = metadata >> COUNT_SHIFT;
        Some(first..first + count)
    }
}

/// Flat hierarchy and stable source-primitive permutation.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Bvh {
    /// Root-first compact nodes.
    pub nodes: Vec<BvhNode>,
    /// Source primitive indices addressed by leaves.
    pub primitive_indices: Vec<u32>,
    /// Per-node stackless-traversal successor, parallel to `nodes`.
    pub escape: Vec<u32>,
}

/// Reusable construction workspace. Keeping it beside a dynamic structure
/// makes trajectory-driven hierarchy rebuilds allocation-free after warmup.
#[derive(Clone, Debug, Default)]
pub struct BvhBuildScratch {
    entries: Vec<MortonEntry>,
    radix: Vec<MortonEntry>,
    stack: Vec<u32>,
}

impl Bvh {
    /// Escape value that ends a stackless traversal.
    pub const ESCAPE_END: u32 = u32::MAX;

    /// Builds a hierarchy over finite, non-empty bounds. Invalid primitives
    /// are omitted instead of poisoning the structure-wide bounds.
    ///
    /// # Errors
    ///
    /// Returns [`BvhBuildError`] when a source row, node or primitive offset
    /// cannot fit the compact GPU representation.
    pub fn build<S: BvhSource + ?Sized>(source: &S) -> Result<Self, BvhBuildError> {
        let mut hierarchy = Self::default();
        hierarchy.rebuild(source, &mut BvhBuildScratch::default())?;
        Ok(hierarchy)
    }

    /// Rebuilds in existing storage, retaining node, permutation and sort
    /// capacities for moving coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`BvhBuildError`] when a source row, node or primitive offset
    /// cannot fit the compact GPU representation.
    pub fn rebuild<S: BvhSource + ?Sized>(
        &mut self,
        source: &S,
        scratch: &mut BvhBuildScratch,
    ) -> Result<(), BvhBuildError> {
        self.nodes.clear();
        self.primitive_indices.clear();
        self.escape.clear();
        scratch.entries.clear();
        let count = checked_u32("BVH source primitive", source.len(), u32::MAX)?;
        // Union is componentwise minimum and maximum: exact, associative and
        // commutative, so splitting it across cores cannot move the result.
        let scene_bounds = build::scene_bounds(source, count);
        if scene_bounds.is_empty() {
            return Ok(());
        }
        build::extend_keys(&mut scratch.entries, source, count, scene_bounds);
        checked_index("BVH primitive table", scratch.entries.len(), INDEX_MASK)?;
        // Entries were pushed in ascending source order and the sort is stable,
        // so ordering by code alone reproduces the `(code, source)` order the
        // hierarchy is specified against without widening the key.
        super::radix::sort_by_code(
            &mut scratch.entries,
            &mut scratch.radix,
            PARALLEL_SORT_THRESHOLD,
        );
        let node_capacity = scratch.entries.len().checked_mul(2).ok_or(BvhBuildError {
            resource: "BVH node capacity",
            index: u64::MAX,
            maximum: INDEX_MASK,
        })?;
        checked_index("BVH node table", node_capacity, INDEX_MASK)?;
        self.nodes.reserve(node_capacity);
        self.primitive_indices.reserve(scratch.entries.len());
        self.nodes.push(BvhNode::default());
        build::build_range(&scratch.entries, source, 0, scratch.entries.len(), 0, self)?;
        self.thread_escapes(&mut scratch.stack);
        Ok(())
    }

    /// Refits leaf and branch bounds without changing topology or primitive
    /// order. The work is `O(resident primitives + nodes)` and reuses every
    /// allocation retained by the hierarchy.
    ///
    /// # Errors
    ///
    /// Returns [`BvhBuildError`] when a primitive used by the existing
    /// topology is absent or no longer has finite, ordered bounds.
    pub fn refit<S: BvhSource + ?Sized>(&mut self, source: &S) -> Result<(), BvhBuildError> {
        for node_index in 0..self.nodes.len() {
            let Some(node) = self.nodes.get(node_index).copied() else {
                continue;
            };
            let Some(range) = node.primitive_range() else {
                continue;
            };
            let first = range.start;
            let count = range.end - range.start;
            let mut aggregate = Aabb::EMPTY;
            let mut maximum_radius = 0.0f32;
            for offset in range {
                let Some(&source_row) = self.primitive_indices.get(offset as usize) else {
                    return Err(refit_error("BVH primitive table", u64::from(offset)));
                };
                if source_row as usize >= source.len() {
                    return Err(refit_error("BVH source primitive", u64::from(source_row)));
                }
                let bound = source.bound(source_row);
                if !build::valid_bound(&bound) {
                    return Err(refit_error("BVH source bounds", u64::from(source_row)));
                }
                aggregate = aggregate.union(&bound);
                maximum_radius = maximum_radius.max(bound.half_extents().max_element());
            }
            self.nodes[node_index] = BvhNode::leaf(aggregate, first, count, maximum_radius);
        }
        for node_index in (0..self.nodes.len()).rev() {
            let Some(node) = self.nodes.get(node_index).copied() else {
                continue;
            };
            let Some((left, right)) = node.children() else {
                continue;
            };
            let Some(left_node) = self.nodes.get(left as usize).copied() else {
                return Err(refit_error("BVH left child", u64::from(left)));
            };
            let Some(right_node) = self.nodes.get(right as usize).copied() else {
                return Err(refit_error("BVH right child", u64::from(right)));
            };
            self.nodes[node_index] = BvhNode::branch(
                left_node.bounds().union(&right_node.bounds()),
                left,
                left_node.maximum_radius().max(right_node.maximum_radius()),
            );
        }
        Ok(())
    }

    /// Threads the finished hierarchy for stackless traversal in `O(nodes)`.
    ///
    /// `escape[n]` names the node a walk moves to once it has skipped or
    /// finished `n`'s subtree. A walk that hits a node descends into its first
    /// child and otherwise follows the link, so no per-ray stack is needed —
    /// which is what makes the hierarchy usable from a fragment shader, where
    /// a stack would cost registers on every lane.
    ///
    /// The left child escapes into its sibling and the sibling inherits the
    /// parent's escape, so the chain leaves any subtree exactly once. The root
    /// escapes to `ESCAPE_END`, which terminates the walk.
    fn thread_escapes(&mut self, stack: &mut Vec<u32>) {
        self.escape.resize(self.nodes.len(), Self::ESCAPE_END);
        stack.clear();
        if self.nodes.is_empty() {
            return;
        }
        stack.push(0);
        while let Some(index) = stack.pop() {
            let Some(node) = self.nodes.get(index as usize).copied() else {
                continue;
            };
            let Some((left, right)) = node.children() else {
                continue;
            };
            let Some(&after) = self.escape.get(index as usize) else {
                continue;
            };
            if let Some(slot) = self.escape.get_mut(left as usize) {
                *slot = right;
            }
            if let Some(slot) = self.escape.get_mut(right as usize) {
                *slot = after;
            }
            stack.push(right);
            stack.push(left);
        }
    }
}

#[inline]
fn refit_error(resource: &'static str, index: u64) -> BvhBuildError {
    BvhBuildError {
        resource,
        index,
        maximum: u32::MAX,
    }
}

/// One primitive's spatial key and the row it came from.
#[derive(Clone, Copy, Debug)]
pub(super) struct MortonEntry {
    pub(super) code: u64,
    pub(super) source: u32,
}

impl MortonEntry {
    /// Fill value for scratch storage the sort is about to overwrite. It is
    /// never observed: every slot is written before it is read back.
    pub(super) const PLACEHOLDER: Self = Self { code: 0, source: 0 };
}
