//! Allocation-reusing spatial queries over a finished hierarchy.

use super::Bvh;
use crate::{Aabb, Vec3};

impl Bvh {
    /// Root bounds, or the empty bound for an empty hierarchy.
    #[must_use]
    #[inline]
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
            self.append_node_candidates(node, traversal, output);
        }
    }

    /// Appends source indices whose primitive bounds overlap a query sphere.
    /// Both caller vectors retain their allocations for reuse.
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
            self.append_node_candidates(node, traversal, output);
        }
    }

    /// Appends source indices whose primitive bounds overlap an axis-aligned
    /// query box. Both caller vectors retain their storage.
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
            self.append_node_candidates(node, traversal, output);
        }
    }

    #[inline]
    fn append_node_candidates(
        &self,
        node: super::BvhNode,
        traversal: &mut Vec<u32>,
        output: &mut Vec<u32>,
    ) {
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

#[inline]
pub(super) fn point_box_distance_squared(point: Vec3, bound: Aabb) -> f32 {
    let outside = (bound.min - point).max(Vec3::ZERO) + (point - bound.max).max(Vec3::ZERO);
    outside.length_squared()
}
