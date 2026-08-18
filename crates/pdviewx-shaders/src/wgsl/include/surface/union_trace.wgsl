// Analytic vdW/SAS tracing through the threaded atom hierarchy.
//
// Traversal is stackless: each node carries the index a walk resumes at once
// its subtree is skipped or finished, so a fragment lane spends no registers
// on a traversal stack. Cost is O(log atoms + intersected atoms) per ray for
// coherent data, and a missed subtree is rejected in one link follow.

fn union_clip_face(
    ray: SurfaceRay,
    clipped: RepresentationClipInterval,
    probe: f32,
) -> SurfaceHit {
    if clipped.entry_plane ==
        NO_CLIP_PLANE ||
        clipped.range.x < 0.0 {
        return surface_miss();
    }

    let hit =
        fma(
            ray.direction,
            vec3f(clipped.range.x),
            ray.origin,
        );

    // One sample supplies both distance and provenance. The original path
    // evaluated union_sample() a second time immediately afterwards.
    let sample =
        union_sample(
            hit,
            probe,
        );

    if sample.compact_index ==
        EMPTY_COMPACT_INDEX ||
        sample.distance >
        representation.surface.y {
        return surface_miss();
    }

    return SurfaceHit(
        hit,
        local_clip_normal(
            clipped.entry_plane
        ),
        sample.compact_index,
        true,
        true,
    );
}

const BVH_ESCAPE_END: u32 = 0xFFFFFFFFu;

// Persistent threaded-BVH escape index.
// Same element count as bvh_nodes.
//
// Contract:
//   - internal node children remain `first` and `first + 1`
//   - bvh_escape[node] points to the next node after this subtree
//   - root escape is BVH_ESCAPE_END
@group(2) @binding(15)
var<storage, read> bvh_escape: array<u32>;

/// Returns the nearest valid intersection with one inflated atom.
///
/// The ray direction is normalized, so the quadratic omits the `a` term.
fn union_atom_hit_t(
    ray: SurfaceRay,
    source_index: u32,
    compact_index: u32,
    probe: f32,
    minimum_t: f32,
) -> f32 {
    let atom =
        atoms[compact_index];

    let base =
        source_index * 3u;

    let center =
        vec3f(
            coords[base],
            coords[base + 1u],
            coords[base + 2u],
        );

    let relative =
        ray.origin - center;

    let projected =
        dot(
            relative,
            ray.direction,
        );

    let radius =
        atom.radius + probe;

    let discriminant =
        projected * projected -
        (
            dot(relative, relative) -
            radius * radius
        );

    if (discriminant < 0.0) {
        return SURFACE_INFINITY;
    }

    let root =
        sqrt(discriminant);

    let near =
        -projected - root;

    if (near >= minimum_t) {
        return near;
    }

    let far =
        -projected + root;

    return select(
        SURFACE_INFINITY,
        far,
        far >= minimum_t,
    );
}

/// Traverses the threaded BVH with O(1) private state.
///
/// A rejected subtree jumps directly to its escape node.
/// An accepted internal node descends into its first child.
/// A processed leaf continues through its escape node.
fn intersect_union_surface(
    ray: SurfaceRay,
) -> SurfaceHit {
    let probe =
        representation.surface.x;

    let root =
        bvh_nodes[0];

    let root_interval =
        ray_box(
            ray.origin,
            ray.inverse_direction,
            root.min_left.xyz -
                vec3f(probe),
            root.max_radius.xyz +
                vec3f(probe),
        );

    let clipped =
        clipped_local_interval(
            ray,
            root_interval,
        );

    let minimum_t =
        max(
            clipped.range.x,
            0.0,
        );

    let maximum_t =
        clipped.range.y;

    if (minimum_t > maximum_t) {
        return surface_miss();
    }

    if (representation.clip_meta.y != 0u) {
        let cap =
            union_clip_face(
                ray,
                clipped,
                probe,
            );

        if (cap.valid) {
            return cap;
        }
    }

    var best_t =
        SURFACE_INFINITY;

    var best_index =
        EMPTY_COMPACT_INDEX;

    var node_index = 0u;

    while (node_index != BVH_ESCAPE_END) {
        let node =
            bvh_nodes[node_index];

        let interval =
            ray_box(
                ray.origin,
                ray.inverse_direction,
                node.min_left.xyz -
                    vec3f(probe),
                node.max_radius.xyz +
                    vec3f(probe),
            );

        // Reject the complete subtree without pushing/popping anything.
        if (
            interval.x > interval.y ||
            interval.y < minimum_t ||
            interval.x > min(best_t, maximum_t)
        ) {
            node_index =
                bvh_escape[node_index];

            continue;
        }

        let metadata =
            bitcast<u32>(
                node.min_left.w
            );

        let count =
            metadata >>
            BVH_COUNT_SHIFT;

        let first =
            metadata &
            BVH_INDEX_MASK;

        // Internal node: descend directly into its first child.
        // That child's escape chain eventually reaches the second child.
        if (count == 0u) {
            node_index =
                first;

            continue;
        }

        // Leaf.
        for (
            var offset = 0u;
            offset < count;
            offset++
        ) {
            let source_index =
                bvh_indices[
                    first + offset
                ];

            let compact_index =
                source_to_compact[
                    source_index
                ];

            if (
                compact_index ==
                EMPTY_COMPACT_INDEX
            ) {
                continue;
            }

            let t =
                union_atom_hit_t(
                    ray,
                    source_index,
                    compact_index,
                    probe,
                    minimum_t,
                );

            if (
                t <= maximum_t &&
                t < best_t
            ) {
                best_t =
                    t;

                best_index =
                    compact_index;
            }
        }

        node_index =
            bvh_escape[node_index];
    }

    if (
        best_index ==
        EMPTY_COMPACT_INDEX
    ) {
        return surface_miss();
    }

    let hit =
        fma(
            ray.direction,
            vec3f(best_t),
            ray.origin,
        );

    let atom =
        atoms[best_index];

    let source_index =
        atom.entity_id &
        0x1FFFFFFFu;

    let base =
        source_index * 3u;

    let center =
        vec3f(
            coords[base],
            coords[base + 1u],
            coords[base + 2u],
        );

    let inverse_radius =
        1.0 /
        max(
            atom.radius + probe,
            SURFACE_RAY_EPSILON,
        );

    return SurfaceHit(
        hit,
        (hit - center) *
            inverse_radius,
        best_index,
        true,
        false,
    );
}
