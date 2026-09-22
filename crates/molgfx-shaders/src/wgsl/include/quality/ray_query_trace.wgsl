// Hardware traversal over the same analytic sphere and capsule records used
// by the portable BVH path. AABBs only produce candidates; exact intersections
// are generated here, so selecting hardware does not change shape semantics.

@group(3) @binding(0) var quality_acceleration: acceleration_structure;

fn quality_candidate_distance(
    primitive_index: u32,
    local_origin: vec3f,
    local_direction: vec3f,
) -> f32 {
    if primitive_index < visual_counts.atoms {
        let atom = atoms[primitive_index];
        let source_index = atom.entity_id & ATOM_SOURCE_MASK;
        let base = source_index * 3u;
        let center = vec3f(coords[base], coords[base + 1u], coords[base + 2u]);
        return quality_sphere_distance(
            local_origin,
            local_direction,
            center,
            atom.radius,
        );
    }
    let bond_index = primitive_index - visual_counts.atoms;
    if bond_index >= visual_counts.bonds {
        return -1.0;
    }
    let bond = quality_bond(bond_index);
    let endpoint_a = atom_position(atoms[bond.atom_a].entity_id);
    let endpoint_b = atom_position(atoms[bond.atom_b].entity_id);
    let local_a = (model.world_to_model * vec4f(endpoint_a, 1.0)).xyz;
    let local_b = (model.world_to_model * vec4f(endpoint_b, 1.0)).xyz;
    return quality_capsule_distance(
        local_origin,
        local_direction,
        local_a,
        local_b,
        bond.radius,
    );
}

fn quality_hit_opacity(
    primitive_index: u32,
    local_origin: vec3f,
    local_direction: vec3f,
    local_distance: f32,
) -> f32 {
    let hit = local_origin + local_direction * local_distance;
    if primitive_index < visual_counts.atoms {
        let atom = atoms[primitive_index];
        let source_index = atom.entity_id & ATOM_SOURCE_MASK;
        let base = source_index * 3u;
        let center = vec3f(coords[base], coords[base + 1u], coords[base + 2u]);
        return quality_hit_transparency(
            atom.entity_id,
            atom_record_color(atom),
            hit,
            normalize(hit - center),
        );
    }
    let bond = quality_bond(primitive_index - visual_counts.atoms);
    let atom_a = atoms[bond.atom_a];
    let atom_b = atoms[bond.atom_b];
    let endpoint_a = atom_position(atom_a.entity_id);
    let endpoint_b = atom_position(atom_b.entity_id);
    let local_a = (model.world_to_model * vec4f(endpoint_a, 1.0)).xyz;
    let local_b = (model.world_to_model * vec4f(endpoint_b, 1.0)).xyz;
    let axis = local_b - local_a;
    let along = clamp(dot(hit - local_a, axis) / max(dot(axis, axis), 1.0e-8), 0.0, 1.0);
    let nearest = local_a + axis * along;
    let entity_id = select(atom_a.entity_id, atom_b.entity_id, along >= 0.5);
    let base_color = mix(
        atom_record_color(atom_a),
        atom_record_color(atom_b),
        along,
    );
    return quality_hit_transparency(
        entity_id,
        base_color,
        hit,
        normalize(hit - nearest),
    );
}

fn trace_transmittance(world_origin: vec3f, world_direction: vec3f, maximum: f32) -> f32 {
    let local_vector = (model.world_to_model * vec4f(world_direction, 0.0)).xyz;
    let local_scale = max(length(local_vector), 1e-6);
    let local_direction = local_vector / local_scale;
    var cursor = 0.0;
    var transmittance = 1.0;
    while cursor < maximum && transmittance > MINIMUM_TRANSMITTANCE {
        let ray_origin = world_origin + world_direction * cursor;
        let local_origin = (model.world_to_model * vec4f(ray_origin, 1.0)).xyz;
        var query: ray_query;
        rayQueryInitialize(
            &query,
            quality_acceleration,
            RayDesc(RAY_FLAG_NONE, 0xffu, 0.0001, maximum - cursor, ray_origin, world_direction),
        );
        while rayQueryProceed(&query) {
            let candidate = rayQueryGetCandidateIntersection(&query);
            if candidate.kind == RAY_QUERY_INTERSECTION_AABB {
                let local_distance = quality_candidate_distance(
                    candidate.primitive_index,
                    local_origin,
                    local_direction,
                );
                let world_distance = local_distance / local_scale;
                if world_distance > 0.0001 && world_distance < maximum - cursor {
                    rayQueryGenerateIntersection(&query, world_distance);
                }
            }
        }
        let committed = rayQueryGetCommittedIntersection(&query);
        if committed.kind == RAY_QUERY_INTERSECTION_NONE {
            break;
        }
        let local_distance = committed.t * local_scale;
        transmittance *= quality_hit_opacity(
            committed.primitive_index,
            local_origin,
            local_direction,
            local_distance,
        );
        cursor += committed.t + 0.0002;
    }
    return select(transmittance, 0.0, transmittance <= MINIMUM_TRANSMITTANCE);
}
