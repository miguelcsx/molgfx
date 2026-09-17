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

// Contract:
//   - internal node children remain `first` and `first + 1`
//   - the escape table follows primitive indices in the shared index buffer
//   - root escape is BVH_ESCAPE_END
fn bvh_escape_index(node: u32) -> u32 {
    let first = arrayLength(&bvh_indices) - arrayLength(&bvh_nodes);
    return bvh_indices[first + node];
}

/// Finds the exact closest selected atom at one resolved field hit.
///
/// Grid provenance used to retain one integer texture beside every scalar
/// field and changed candidates at voxel faces. One stackless BVH query at the
/// final hit removes both full-resolution provenance textures and makes colour
/// ownership independent of the grid.
fn surface_nearest_atom(point: vec3f) -> u32 {
    var best_distance = SURFACE_INFINITY;
    var best_index = EMPTY_COMPACT_INDEX;
    var node_index = 0u;

    while node_index != BVH_ESCAPE_END {
        let node = bvh_nodes[node_index];
        let lower_bound = distance_to_box(
            point,
            node.min_left.xyz,
            node.max_radius.xyz,
        ) - node.max_radius.w * representation.visual.w;

        if lower_bound > best_distance {
            node_index = bvh_escape_index(node_index);
            continue;
        }

        let metadata = bitcast<u32>(node.min_left.w);
        let count = metadata >> BVH_COUNT_SHIFT;
        let first = metadata & BVH_INDEX_MASK;

        if count == 0u {
            node_index = first;
            continue;
        }

        for (var offset = 0u; offset < count; offset++) {
            let source_index = bvh_indices[first + offset];
            let compact_index = source_to_compact[source_index];

            if compact_index == EMPTY_COMPACT_INDEX {
                continue;
            }

            let atom = atoms[compact_index];
            let base = source_index * 3u;
            let center = vec3f(
                coords[base],
                coords[base + 1u],
                coords[base + 2u],
            );
            let distance = length(point - center)
                - atom.radius * representation.visual.w;

            if distance < best_distance {
                best_distance = distance;
                best_index = compact_index;
            }
        }

        node_index = bvh_escape_index(node_index);
    }

    return best_index;
}

/// Returns the nearest valid intersection with one inflated atom.
///
/// The ray direction is normalized, so the quadratic omits the `a` term.
fn union_atom_hit_t(
    ray: SurfaceRay,
    source_index: u32,
    compact_index: u32,
    inflation: f32,
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
        atom.radius * representation.visual.w + inflation;

    if radius <= SURFACE_RAY_EPSILON {
        return SURFACE_INFINITY;
    }

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

    let inflation =
        probe + representation.surface.y;

    let root =
        bvh_nodes[0];

    let root_interval =
        ray_box(
            ray.origin,
            ray.inverse_direction,
            root.min_left.xyz -
                vec3f(surface_node_padding(root)),
            root.max_radius.xyz +
                vec3f(surface_node_padding(root)),
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

    var second_t =
        SURFACE_INFINITY;

    var second_index =
        EMPTY_COMPACT_INDEX;

    var third_t = SURFACE_INFINITY;
    var third_index = EMPTY_COMPACT_INDEX;
    var fourth_t = SURFACE_INFINITY;
    var fourth_index = EMPTY_COMPACT_INDEX;

    var node_index = 0u;

    let soft_union = probe > 0.0 && representation.options.w == 5u;
    let normal_blend_span = select(0.0, 2.0, soft_union);

    while (node_index != BVH_ESCAPE_END) {
        let node =
            bvh_nodes[node_index];

        let interval =
            ray_box(
                ray.origin,
                ray.inverse_direction,
                node.min_left.xyz -
                    vec3f(surface_node_padding(node)),
                node.max_radius.xyz +
                    vec3f(surface_node_padding(node)),
            );

        // Reject the complete subtree without pushing/popping anything.
        if (
            interval.x > interval.y ||
            interval.y < minimum_t ||
            interval.x > min(best_t + normal_blend_span, maximum_t)
        ) {
            node_index =
                bvh_escape_index(node_index);

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
                    inflation,
                    minimum_t,
                );

            if (
                t <= maximum_t &&
                t < best_t
            ) {
                fourth_t = third_t;
                fourth_index = third_index;
                third_t = second_t;
                third_index = second_index;
                second_t = best_t;
                second_index = best_index;
                best_t =
                    t;

                best_index =
                    compact_index;
            } else if (
                t <= maximum_t &&
                t < second_t
            ) {
                fourth_t = third_t;
                fourth_index = third_index;
                third_t = second_t;
                third_index = second_index;
                second_t = t;
                second_index = compact_index;
            } else if (t <= maximum_t && t < third_t) {
                fourth_t = third_t;
                fourth_index = third_index;
                third_t = t;
                third_index = compact_index;
            } else if (t <= maximum_t && t < fourth_t) {
                fourth_t = t;
                fourth_index = compact_index;
            }
        }

        node_index =
            bvh_escape_index(node_index);
    }

    if (
        best_index ==
        EMPTY_COMPACT_INDEX
    ) {
        return surface_miss();
    }

    var resolved_t = best_t;

    let nearest_parameters = array<f32, 4>(best_t, second_t, third_t, fourth_t);
    let nearest_indices = array<u32, 4>(
        best_index,
        second_index,
        third_index,
        fourth_index,
    );

    if soft_union && second_index != EMPTY_COMPACT_INDEX {
        resolved_t = soft_union_parameter(nearest_parameters, normal_blend_span);
    }

    let hit =
        fma(
            ray.direction,
            vec3f(resolved_t),
            ray.origin,
        );

    var normal = union_atom_surface_normal(hit, best_index, inflation);

    // SoftUnion is an explicit illustrative preview, distinct from exact
    // Solid SAS. It applies a two-nearest polynomial soft minimum and the
    // matching normal blend, rounding intersection cusps without a voxel grid.
    // Exact SAS and vdW keep their literal sphere-union position and normal.
    if (
        soft_union &&
        second_index != EMPTY_COMPACT_INDEX
    ) {
        normal = soft_union_surface_normal(
            hit,
            nearest_parameters,
            nearest_indices,
            normal_blend_span,
            inflation,
        );
    }

    return SurfaceHit(
        hit,
        normal,
        best_index,
        true,
        false,
    );
}
