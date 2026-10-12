// Procedural probe-inflated atomic samples, rejecting overlaps in the resident
// spatial hierarchy. This samples an accessible boundary; it measures no SASA.
//!include "include/surface_field.wgsl"

fn point_solvent_visible(source: AtomRecord, point: vec3f) -> bool {
    var stack: array<u32, 64>;
    var count = 1u;
    stack[0] = 0u;
    let probe = representation.surface.x;
    while count > 0u {
        count -= 1u;
        let node = bvh_nodes[stack[count]];
        if distance_to_box(point, node.min_left.xyz, node.max_radius.xyz) > node.max_radius.w * representation.visual.y + probe {
            continue;
        }
        let metadata = bitcast<u32>(node.min_left.w);
        let children = metadata >> BVH_COUNT_SHIFT;
        let first = metadata & BVH_INDEX_MASK;
        if children == 0u {
            stack[count] = first;
            stack[count + 1u] = first + 1u;
            count += 2u;
        } else {
            for (var index = 0u; index < children; index += 1u) {
                let row = bvh_indices[first + index];
                let compact = source_to_compact[row];
                if compact == EMPTY_COMPACT_INDEX { continue; }
                let other = atoms[compact];
                if other.entity_id == source.entity_id { continue; }
                let center = atom_local_position(row * 3u);
                let delta = point - center;
                let radius = other.radius;
                if dot(delta, delta) < radius * radius - 1e-5 { return false; }
            }
        }
    }
    return true;
}
