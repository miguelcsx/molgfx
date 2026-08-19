//! Deterministic linear bounding-volume hierarchy.
//!
//! Construction Morton-sorts `n` primitive bounds in `O(n log n)` and emits
//! `O(n)` compact nodes. Traversal is `O(log n + hits)` for coherent spatial
//! data and accepts caller-owned scratch vectors, so repeated queries allocate
//! nothing. The 32-byte node is directly uploadable to storage buffers.

use crate::{Aabb, Vec3};
use std::ops::Range;

#[cfg(test)]
#[path = "bvh_tests.rs"]
mod tests;

const LEAF_SIZE: usize = 4;
const COUNT_SHIFT: u32 = 29;
const INDEX_MASK: u32 = (1 << COUNT_SHIFT) - 1;

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
    fn leaf(bounds: Aabb, first: u32, count: u32, max_radius: f32) -> Self {
        let metadata = (first & INDEX_MASK) | (count.min(7) << COUNT_SHIFT);
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

    fn branch(bounds: Aabb, left: u32, max_radius: f32) -> Self {
        Self {
            min_left: [
                bounds.min.x,
                bounds.min.y,
                bounds.min.z,
                f32::from_bits(left & INDEX_MASK),
            ],
            max_radius: [bounds.max.x, bounds.max.y, bounds.max.z, max_radius],
        }
    }

    /// Node bounds.
    #[must_use]
    pub fn bounds(self) -> Aabb {
        Aabb::new(
            Vec3::from_array([self.min_left[0], self.min_left[1], self.min_left[2]]),
            Vec3::from_array([self.max_radius[0], self.max_radius[1], self.max_radius[2]]),
        )
    }

    /// Largest primitive half-extent below this node.
    #[must_use]
    pub fn maximum_radius(self) -> f32 {
        self.max_radius[3]
    }

    /// True for a leaf node.
    #[must_use]
    pub fn is_leaf(self) -> bool {
        self.min_left[3].to_bits() >> COUNT_SHIFT != 0
    }

    /// Adjacent child indices for an internal node.
    #[must_use]
    pub fn children(self) -> Option<(u32, u32)> {
        if self.is_leaf() {
            return None;
        }
        let left = self.min_left[3].to_bits() & INDEX_MASK;
        Some((left, left.saturating_add(1)))
    }

    /// Range into `Bvh::primitive_indices` for a leaf.
    #[must_use]
    pub fn primitive_range(self) -> Option<Range<u32>> {
        if !self.is_leaf() {
            return None;
        }
        let metadata = self.min_left[3].to_bits();
        let first = metadata & INDEX_MASK;
        let count = metadata >> COUNT_SHIFT;
        Some(first..first.saturating_add(count))
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
#[derive(Debug, Default)]
pub struct BvhBuildScratch {
    entries: Vec<MortonEntry>,
    stack: Vec<u32>,
}

impl Bvh {
    /// Escape value that ends a stackless traversal.
    pub const ESCAPE_END: u32 = u32::MAX;

    /// Builds a hierarchy over finite, non-empty bounds. Invalid primitives
    /// are omitted instead of poisoning the structure-wide bounds.
    #[must_use]
    pub fn build(bounds: &[Aabb]) -> Self {
        let mut hierarchy = Self::default();
        hierarchy.rebuild(bounds, &mut BvhBuildScratch::default());
        hierarchy
    }

    /// Rebuilds in existing storage, retaining node, permutation and sort
    /// capacities for moving coordinates.
    pub fn rebuild(&mut self, bounds: &[Aabb], scratch: &mut BvhBuildScratch) {
        self.nodes.clear();
        self.primitive_indices.clear();
        self.escape.clear();
        scratch.entries.clear();
        let scene_bounds = bounds
            .iter()
            .filter(|bound| valid_bound(bound))
            .fold(Aabb::EMPTY, |acc, bound| acc.union(bound));
        if scene_bounds.is_empty() {
            return;
        }
        scratch.entries.extend(
            bounds
                .iter()
                .enumerate()
                .filter(|(_, bound)| valid_bound(bound))
                .map(|(source, bound)| MortonEntry {
                    code: morton_code(bound.center(), scene_bounds),
                    source: u32::try_from(source).map_or(u32::MAX, |value| value),
                })
                .filter(|entry| entry.source != u32::MAX),
        );
        scratch
            .entries
            .sort_unstable_by_key(|entry| (entry.code, entry.source));
        self.nodes.reserve(scratch.entries.len().saturating_mul(2));
        self.primitive_indices.reserve(scratch.entries.len());
        self.nodes.push(BvhNode::default());
        build_range(&scratch.entries, bounds, 0, scratch.entries.len(), 0, self);
        self.thread_escapes(&mut scratch.stack);
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

    /// Root bounds, or the empty bound for an empty hierarchy.
    #[must_use]
    pub fn bounds(&self) -> Aabb {
        self.nodes.first().map_or(Aabb::EMPTY, |node| node.bounds())
    }

    /// Appends source indices whose bounds overlap a ray. Both vectors are
    /// cleared and reused; results follow deterministic hierarchy order.
    pub fn ray_candidates(
        &self,
        origin: Vec3,
        direction: Vec3,
        traversal: &mut Vec<u32>,
        output: &mut Vec<u32>,
    ) {
        traversal.clear();
        output.clear();
        if self.nodes.is_empty() || !origin.is_finite() || !direction.is_finite() {
            return;
        }
        traversal.push(0);
        let inverse_direction = direction.recip();
        while let Some(index) = traversal.pop() {
            let Some(node) = self.nodes.get(index as usize).copied() else {
                continue;
            };
            if node
                .bounds()
                .ray_intersect(origin, inverse_direction)
                .is_none()
            {
                continue;
            }
            if let Some(range) = node.primitive_range() {
                let start = range.start as usize;
                let end = range.end as usize;
                if let Some(indices) = self.primitive_indices.get(start..end) {
                    output.extend_from_slice(indices);
                }
            } else if let Some((left, right)) = node.children() {
                traversal.push(right);
                traversal.push(left);
            }
        }
    }

    /// Appends source indices whose primitive bounds overlap a query sphere.
    /// Traversal is `O(log n + candidates)` for coherent spatial data. Both
    /// caller vectors are cleared and retain their allocations for reuse.
    pub fn sphere_candidates(
        &self,
        center: Vec3,
        radius: f32,
        traversal: &mut Vec<u32>,
        output: &mut Vec<u32>,
    ) {
        traversal.clear();
        output.clear();
        if self.nodes.is_empty() || !center.is_finite() || !radius.is_finite() || radius < 0.0 {
            return;
        }
        traversal.push(0);
        let radius_sq = radius * radius;
        while let Some(index) = traversal.pop() {
            let Some(node) = self.nodes.get(index as usize).copied() else {
                continue;
            };
            if point_box_distance_squared(center, node.bounds()) > radius_sq {
                continue;
            }
            if let Some(range) = node.primitive_range() {
                let start = range.start as usize;
                let end = range.end as usize;
                if let Some(indices) = self.primitive_indices.get(start..end) {
                    output.extend_from_slice(indices);
                }
            } else if let Some((left, right)) = node.children() {
                traversal.push(right);
                traversal.push(left);
            }
        }
    }

    /// Appends source indices whose primitive bounds overlap an axis-aligned
    /// query box. Both caller vectors are cleared and retain their storage.
    pub fn aabb_candidates(&self, query: Aabb, traversal: &mut Vec<u32>, output: &mut Vec<u32>) {
        traversal.clear();
        output.clear();
        if self.nodes.is_empty()
            || query.is_empty()
            || !query.min.is_finite()
            || !query.max.is_finite()
        {
            return;
        }
        traversal.push(0);
        while let Some(index) = traversal.pop() {
            let Some(node) = self.nodes.get(index as usize).copied() else {
                continue;
            };
            if !node.bounds().overlaps(&query) {
                continue;
            }
            if let Some(range) = node.primitive_range() {
                let start = range.start as usize;
                let end = range.end as usize;
                if let Some(indices) = self.primitive_indices.get(start..end) {
                    output.extend_from_slice(indices);
                }
            } else if let Some((left, right)) = node.children() {
                traversal.push(right);
                traversal.push(left);
            }
        }
    }
}

fn point_box_distance_squared(point: Vec3, bound: Aabb) -> f32 {
    let outside = (bound.min - point).max(Vec3::ZERO) + (point - bound.max).max(Vec3::ZERO);
    outside.length_squared()
}

#[derive(Clone, Copy, Debug)]
struct MortonEntry {
    code: u32,
    source: u32,
}

fn build_range(
    entries: &[MortonEntry],
    source_bounds: &[Aabb],
    start: usize,
    end: usize,
    node: usize,
    out: &mut Bvh,
) -> BuildAggregate {
    if end - start <= LEAF_SIZE {
        let first = u32::try_from(out.primitive_indices.len()).map_or(u32::MAX, |value| value);
        let mut bounds = Aabb::EMPTY;
        let mut max_radius = 0.0f32;
        for entry in &entries[start..end] {
            let Some(bound) = source_bounds.get(entry.source as usize) else {
                continue;
            };
            bounds = bounds.union(bound);
            max_radius = max_radius.max(bound.half_extents().max_element());
            out.primitive_indices.push(entry.source);
        }
        let count = u32::try_from(end - start).map_or(u32::MAX, |value| value);
        out.nodes[node] = BvhNode::leaf(bounds, first, count, max_radius);
        return BuildAggregate { bounds, max_radius };
    }

    let split = split_range(entries, start, end);
    let left = out.nodes.len();
    out.nodes.extend([BvhNode::default(), BvhNode::default()]);
    let left_bounds = build_range(entries, source_bounds, start, split, left, out);
    let right_bounds = build_range(entries, source_bounds, split, end, left + 1, out);
    let bounds = left_bounds.bounds.union(&right_bounds.bounds);
    let max_radius = left_bounds.max_radius.max(right_bounds.max_radius);
    let left = u32::try_from(left).map_or(u32::MAX, |value| value);
    out.nodes[node] = BvhNode::branch(bounds, left, max_radius);
    BuildAggregate { bounds, max_radius }
}

#[derive(Clone, Copy)]
struct BuildAggregate {
    bounds: Aabb,
    max_radius: f32,
}

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

fn valid_bound(bound: &Aabb) -> bool {
    !bound.is_empty() && bound.min.is_finite() && bound.max.is_finite()
}

fn morton_code(point: Vec3, bounds: Aabb) -> u32 {
    let extent = (bounds.max - bounds.min).max(Vec3::splat(f32::EPSILON));
    let unit = ((point - bounds.min) / extent).clamp(Vec3::ZERO, Vec3::ONE);
    let x = quantize_10(unit.x);
    let y = quantize_10(unit.y);
    let z = quantize_10(unit.z);
    expand_10(x) | (expand_10(y) << 1) | (expand_10(z) << 2)
}

fn quantize_10(value: f32) -> u32 {
    let target = value * 1023.0;
    let mut low = 0u16;
    let mut high = 1023u16;
    while low < high {
        let middle = (low + high).div_ceil(2);
        if f32::from(middle) <= target {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    u32::from(low)
}

const fn expand_10(mut value: u32) -> u32 {
    value &= 0x0000_03ff;
    value = (value | (value << 16)) & 0x0300_00ff;
    value = (value | (value << 8)) & 0x0300_f00f;
    value = (value | (value << 4)) & 0x030c_30c3;
    value = (value | (value << 2)) & 0x0924_9249;
    value
}
