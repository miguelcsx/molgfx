// Shared compact-BVH molecular field evaluation.

const BVH_COUNT_SHIFT: u32 = 29u;
const BVH_INDEX_MASK: u32 = 0x1FFFFFFFu;
const EMPTY_COMPACT_INDEX: u32 = 0xFFFFFFFFu;
struct BvhNode {
    min_left: vec4f,
    max_radius: vec4f,
}

//!include "include/representation.wgsl"

struct DistanceSample {
    distance: f32,
    compact_index: u32,
}

// Four standard deviations bound the Gaussian support used by the voxel
// generator. The compact support keeps each voxel's BVH traversal finite while
// preserving the visible boundary for the supported positive iso-levels.
const GAUSSIAN_SUPPORT_SIGMAS: f32 = 4.0;

@group(2) @binding(6) var<storage, read> bvh_nodes: array<BvhNode>;
@group(2) @binding(7) var<storage, read> bvh_indices: array<u32>;
@group(2) @binding(8) var<storage, read> source_to_compact: array<u32>;

fn distance_to_box(point: vec3f, lower: vec3f, upper: vec3f) -> f32 {
    return length(max(max(lower - point, point - upper), vec3f(0.0)));
}

fn union_sample(point: vec3f, probe: f32) -> DistanceSample {
    var best = DistanceSample(1e20, EMPTY_COMPACT_INDEX);
    var stack: array<u32, 64>;
    var stack_size = 1u;
    stack[0] = 0u;
    while stack_size > 0u {
        stack_size -= 1u;
        let node_index = stack[stack_size];
        if node_index >= arrayLength(&bvh_nodes) {
            continue;
        }
        let node = bvh_nodes[node_index];
        let lower_bound = distance_to_box(point, node.min_left.xyz, node.max_radius.xyz)
            - node.max_radius.w * representation.visual.w - probe;
        if lower_bound > best.distance {
            continue;
        }
        let metadata = bitcast<u32>(node.min_left.w);
        let count = metadata >> BVH_COUNT_SHIFT;
        let first = metadata & BVH_INDEX_MASK;
        if count > 0u {
            for (var offset = 0u; offset < count; offset += 1u) {
                let source_index = bvh_indices[first + offset];
                let compact_index = source_to_compact[source_index];
                if compact_index == EMPTY_COMPACT_INDEX {
                    continue;
                }
                let atom = atoms[compact_index];
                let base = source_index * 3u;
                let center = vec3f(coords[base], coords[base + 1u], coords[base + 2u]);
                let distance = length(point - center)
                    - atom.radius * representation.visual.w
                    - probe;
                if distance < best.distance {
                    best = DistanceSample(distance, compact_index);
                }
            }
        } else {
            let left = first;
            if stack_size + 2u <= 64u {
                stack[stack_size] = left + 1u;
                stack[stack_size + 1u] = left;
                stack_size += 2u;
            }
        }
    }
    return best;
}

// Atom-centred Gaussian density used for publication-style molecular surfaces.
// The value is a unit-weight sum, so the caller's iso-level remains a direct
// and reversible density control rather than a distance disguised as one.
fn gaussian_sample(point: vec3f) -> DistanceSample {
    let sigma = max(representation.surface.z, 1e-4);
    let support = max(representation.surface.x, sigma * GAUSSIAN_SUPPORT_SIGMAS);
    var density = 0.0;
    var nearest = EMPTY_COMPACT_INDEX;
    var nearest_distance = 1e20;
    var stack: array<u32, 64>;
    var stack_size = 1u;
    stack[0] = 0u;
    while stack_size > 0u {
        stack_size -= 1u;
        let node_index = stack[stack_size];
        if node_index >= arrayLength(&bvh_nodes) {
            continue;
        }
        let node = bvh_nodes[node_index];
        let lower_bound = distance_to_box(point, node.min_left.xyz, node.max_radius.xyz);
        if lower_bound > support {
            continue;
        }
        let metadata = bitcast<u32>(node.min_left.w);
        let count = metadata >> BVH_COUNT_SHIFT;
        let first = metadata & BVH_INDEX_MASK;
        if count > 0u {
            for (var offset = 0u; offset < count; offset += 1u) {
                let source_index = bvh_indices[first + offset];
                let compact_index = source_to_compact[source_index];
                if compact_index == EMPTY_COMPACT_INDEX {
                    continue;
                }
                let base = source_index * 3u;
                let center = vec3f(coords[base], coords[base + 1u], coords[base + 2u]);
                let distance = length(point - center);
                if distance > support {
                    continue;
                }
                let normalized = distance / sigma;
                density += exp(-0.5 * normalized * normalized);
                if distance < nearest_distance {
                    nearest_distance = distance;
                    nearest = compact_index;
                }
            }
        } else if stack_size + 2u <= 64u {
            let left = first;
            stack[stack_size] = left + 1u;
            stack[stack_size + 1u] = left;
            stack_size += 2u;
        }
    }
    return DistanceSample(density, nearest);
}
