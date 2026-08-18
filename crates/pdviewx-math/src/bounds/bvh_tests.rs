use super::*;
use std::collections::BTreeSet;

fn box_at(center: Vec3, radius: f32) -> Aabb {
    Aabb::new(center - Vec3::splat(radius), center + Vec3::splat(radius))
}

#[test]
fn a_gpu_node_is_exactly_two_aligned_vec4_records() {
    assert_eq!(std::mem::size_of::<BvhNode>(), 32);
    assert_eq!(std::mem::align_of::<BvhNode>(), 16);
}

#[test]
fn node_radius_bounds_every_primitive_below_it() {
    let hierarchy = Bvh::build(&[
        box_at(Vec3::ZERO, 0.5),
        box_at(Vec3::new(4.0, 0.0, 0.0), 2.0),
        box_at(Vec3::new(-4.0, 0.0, 0.0), 1.0),
    ]);
    let Some(root) = hierarchy.nodes.first() else {
        panic!("hierarchy has a root")
    };
    assert!((root.maximum_radius() - 2.0).abs() < f32::EPSILON);
}

#[test]
fn an_empty_or_invalid_input_builds_an_empty_hierarchy() {
    assert_eq!(Bvh::build(&[]), Bvh::default());
    assert_eq!(Bvh::build(&[Aabb::EMPTY]), Bvh::default());
    let invalid = Aabb::new(Vec3::splat(f32::NAN), Vec3::ONE);
    assert_eq!(Bvh::build(&[invalid]), Bvh::default());
}

#[test]
fn every_valid_primitive_appears_in_exactly_one_leaf() {
    let bounds = (0u16..97)
        .map(|index| box_at(Vec3::new(f32::from(index), f32::from(index % 7), 0.0), 0.4))
        .collect::<Vec<_>>();
    let hierarchy = Bvh::build(&bounds);
    let actual = hierarchy
        .primitive_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let expected = (0..97).collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(hierarchy.primitive_indices.len(), bounds.len());
}

#[test]
fn construction_is_byte_deterministic_for_equal_centroids() {
    let bounds = vec![box_at(Vec3::ZERO, 1.0); 19];
    let first = Bvh::build(&bounds);
    let second = Bvh::build(&bounds);
    assert_eq!(first, second);
    assert_eq!(first.primitive_indices, (0..19).collect::<Vec<_>>());
}

#[test]
fn rebuilding_after_warmup_reuses_every_construction_allocation() {
    let bounds = (0u16..128)
        .map(|index| box_at(Vec3::new(f32::from(index), 0.0, 0.0), 0.5))
        .collect::<Vec<_>>();
    let mut hierarchy = Bvh::default();
    let mut scratch = BvhBuildScratch::default();
    hierarchy.rebuild(&bounds, &mut scratch);
    let capacities = (
        hierarchy.nodes.capacity(),
        hierarchy.primitive_indices.capacity(),
        scratch.entries.capacity(),
    );
    hierarchy.rebuild(&bounds, &mut scratch);
    assert_eq!(
        (
            hierarchy.nodes.capacity(),
            hierarchy.primitive_indices.capacity(),
            scratch.entries.capacity(),
        ),
        capacities
    );
}

#[test]
fn every_branch_contains_both_children() {
    let bounds = (0u16..31)
        .map(|index| box_at(Vec3::new(f32::from(index), f32::from(index % 3), 1.0), 0.5))
        .collect::<Vec<_>>();
    let hierarchy = Bvh::build(&bounds);
    for node in &hierarchy.nodes {
        let Some((left, right)) = node.children() else {
            continue;
        };
        let Some(left) = hierarchy.nodes.get(left as usize) else {
            panic!("left child exists")
        };
        let Some(right) = hierarchy.nodes.get(right as usize) else {
            panic!("right child exists")
        };
        assert_contains(node.bounds(), left.bounds());
        assert_contains(node.bounds(), right.bounds());
    }
}

#[test]
fn ray_candidates_match_brute_force_leaf_bounds() {
    let bounds = vec![
        box_at(Vec3::new(0.0, 0.0, 0.0), 0.5),
        box_at(Vec3::new(0.0, 0.0, -3.0), 0.5),
        box_at(Vec3::new(4.0, 0.0, -2.0), 0.5),
        box_at(Vec3::new(-4.0, 0.0, -2.0), 0.5),
        box_at(Vec3::new(0.0, 4.0, -2.0), 0.5),
    ];
    let hierarchy = Bvh::build(&bounds);
    let origin = Vec3::new(0.0, 0.0, 4.0);
    let direction = Vec3::new(0.0, 0.0, -1.0);
    let mut traversal = Vec::new();
    let mut candidates = Vec::new();
    hierarchy.ray_candidates(origin, direction, &mut traversal, &mut candidates);
    candidates.retain(|index| {
        bounds[*index as usize]
            .ray_intersect(origin, direction.recip())
            .is_some()
    });
    candidates.sort_unstable();
    assert_eq!(candidates, vec![0, 1]);

    let traversal_capacity = traversal.capacity();
    let output_capacity = candidates.capacity();
    hierarchy.ray_candidates(origin, direction, &mut traversal, &mut candidates);
    assert_eq!(traversal.capacity(), traversal_capacity);
    assert_eq!(candidates.capacity(), output_capacity);
}

#[test]
fn sphere_candidates_prune_distant_subtrees_and_reuse_storage() {
    let bounds = (0u16..96)
        .map(|index| box_at(Vec3::new(f32::from(index), 0.0, 0.0), 0.25))
        .collect::<Vec<_>>();
    let hierarchy = Bvh::build(&bounds);
    let mut traversal = Vec::new();
    let mut candidates = Vec::new();
    hierarchy.sphere_candidates(
        Vec3::new(48.0, 0.0, 0.0),
        1.1,
        &mut traversal,
        &mut candidates,
    );
    candidates.retain(|index| {
        point_box_distance_squared(Vec3::new(48.0, 0.0, 0.0), bounds[*index as usize]) <= 1.21
    });
    candidates.sort_unstable();
    assert_eq!(candidates, vec![47, 48, 49]);
    let capacities = (traversal.capacity(), candidates.capacity());
    hierarchy.sphere_candidates(
        Vec3::new(48.0, 0.0, 0.0),
        1.1,
        &mut traversal,
        &mut candidates,
    );
    assert_eq!((traversal.capacity(), candidates.capacity()), capacities);
}

#[test]
fn aabb_candidates_prune_distant_subtrees_and_reuse_storage() {
    let bounds = (0u16..96)
        .map(|index| box_at(Vec3::new(f32::from(index), 0.0, 0.0), 0.25))
        .collect::<Vec<_>>();
    let hierarchy = Bvh::build(&bounds);
    let mut traversal = Vec::new();
    let mut candidates = Vec::new();
    hierarchy.aabb_candidates(
        Aabb::new(Vec3::new(47.25, -0.5, -0.5), Vec3::new(48.7, 0.5, 0.5)),
        &mut traversal,
        &mut candidates,
    );
    candidates.retain(|index| {
        bounds[*index as usize].overlaps(&Aabb::new(
            Vec3::new(47.25, -0.5, -0.5),
            Vec3::new(48.7, 0.5, 0.5),
        ))
    });
    candidates.sort_unstable();
    assert_eq!(candidates, vec![47, 48]);
    let capacities = (traversal.capacity(), candidates.capacity());
    hierarchy.aabb_candidates(
        Aabb::new(Vec3::new(47.25, -0.5, -0.5), Vec3::new(48.7, 0.5, 0.5)),
        &mut traversal,
        &mut candidates,
    );
    assert_eq!((traversal.capacity(), candidates.capacity()), capacities);
}

fn assert_contains(parent: Aabb, child: Aabb) {
    assert!(parent.min.cmple(child.min).all());
    assert!(parent.max.cmpge(child.max).all());
}

#[test]
fn a_stackless_escape_walk_reaches_every_primitive_exactly_once() {
    let bounds = (0..97i16)
        .map(|index| box_at(Vec3::new(f32::from(index), 0.0, 0.0), 0.4))
        .collect::<Vec<_>>();
    let hierarchy = Bvh::build(&bounds);
    assert_eq!(hierarchy.escape.len(), hierarchy.nodes.len());

    // Mirrors the shader walk: descend on a hit, follow the link otherwise.
    let mut visited = Vec::new();
    let mut node = 0u32;
    while node != Bvh::ESCAPE_END {
        let Some(current) = hierarchy.nodes.get(node as usize).copied() else {
            panic!("escape chain left the node array at index {node}");
        };
        if let Some(range) = current.primitive_range() {
            let start = range.start as usize;
            let end = range.end as usize;
            let Some(indices) = hierarchy.primitive_indices.get(start..end) else {
                panic!("leaf at index {node} addresses primitives out of range");
            };
            visited.extend_from_slice(indices);
            node = hierarchy.escape[node as usize];
        } else {
            let Some((left, _)) = current.children() else {
                panic!("internal node at index {node} has no children");
            };
            node = left;
        }
    }
    visited.sort_unstable();
    assert_eq!(visited, (0..97).collect::<Vec<u32>>());
}

#[test]
fn skipping_the_root_subtree_ends_a_stackless_walk() {
    let bounds = (0..40i16)
        .map(|index| box_at(Vec3::new(0.0, f32::from(index), 0.0), 0.4))
        .collect::<Vec<_>>();
    let hierarchy = Bvh::build(&bounds);
    assert_eq!(hierarchy.escape.first(), Some(&Bvh::ESCAPE_END));
}

#[test]
fn rebuilding_retains_the_escape_column_allocation() {
    let bounds = (0..64i16)
        .map(|index| box_at(Vec3::new(f32::from(index), 0.0, 0.0), 0.4))
        .collect::<Vec<_>>();
    let mut scratch = BvhBuildScratch::default();
    let mut hierarchy = Bvh::default();
    hierarchy.rebuild(&bounds, &mut scratch);
    let capacity = hierarchy.escape.capacity();
    hierarchy.rebuild(&bounds, &mut scratch);
    assert_eq!(hierarchy.escape.capacity(), capacity);
    assert_eq!(hierarchy.escape.len(), hierarchy.nodes.len());
}
