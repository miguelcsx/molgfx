/// Fraction of light that survives the ray.
fn trace_transmittance(world_origin: vec3f, world_direction: vec3f, maximum: f32) -> f32 {
    let origin = (model.world_to_model * vec4f(world_origin, 1.0)).xyz;
    let local_vector = (model.world_to_model * vec4f(world_direction, 0.0)).xyz;
    let local_scale = max(length(local_vector), 1e-6);
    let direction = local_vector / local_scale;
    let local_maximum = maximum * local_scale;
    let inverse_direction = 1.0 / direction;
    var transmittance = 1.0;
    let atom_node_count = visual_counts.atom_bvh_nodes;
    let atom_escape_base = quality_escape_base(visual_counts.atom_bvh_indices, atom_node_count);
    var node_index = select(QUALITY_TRAVERSAL_END, 0u, atom_node_count > 0u);
    while node_index != QUALITY_TRAVERSAL_END && node_index < atom_node_count {
        let node = bvh_nodes[node_index];
        let escape = bvh_indices[atom_escape_base + node_index];
        let interval = quality_node_interval(origin, inverse_direction, node, node.max_radius.w);
        if quality_node_is_missed(interval, local_maximum) {
            node_index = escape;
            continue;
        }
        let count = quality_node_count(node);
        let first = quality_node_first(node);
        if count == 0u {
            node_index = first;
            continue;
        }
        for (var offset = 0u; offset < count; offset += 1u) {
            let source_index = bvh_indices[first + offset];
            let compact_index = source_to_compact[source_index];
            if compact_index == EMPTY_COMPACT_INDEX {
                continue;
            }
            let atom = atoms[compact_index];
            let base = source_index * 3u;
            let center = vec3f(coords[base], coords[base + 1u], coords[base + 2u]);
            let distance = quality_sphere_distance(origin, direction, center, atom.radius);
            if distance > 0.0 && distance < local_maximum {
                let hit = origin + direction * distance;
                transmittance *= quality_hit_transparency(
                    atom.entity_id,
                    atom_visual_color(atom.entity_id, atom.color),
                    hit,
                    normalize(hit - center),
                );
                if transmittance <= MINIMUM_TRANSMITTANCE {
                    return 0.0;
                }
            }
        }
        node_index = escape;
    }
    let bond_node_count = visual_counts.bond_bvh_nodes;
    let bond_escape_base = quality_escape_base(visual_counts.bond_bvh_indices, bond_node_count);
    node_index = select(QUALITY_TRAVERSAL_END, 0u, bond_node_count > 0u);
    while node_index != QUALITY_TRAVERSAL_END && node_index < bond_node_count {
        let node = quality_bond_node(node_index);
        let escape = quality_bond_index(bond_escape_base + node_index);
        let interval = quality_node_interval(origin, inverse_direction, node, 0.0);
        if quality_node_is_missed(interval, local_maximum) {
            node_index = escape;
            continue;
        }
        let count = quality_node_count(node);
        let first = quality_node_first(node);
        if count == 0u {
            node_index = first;
            continue;
        }
        for (var offset = 0u; offset < count; offset += 1u) {
            let bond_index = quality_bond_index(first + offset);
            let bond = quality_bond(bond_index);
            let endpoint_a = atom_position(atoms[bond.atom_a].entity_id);
            let endpoint_b = atom_position(atoms[bond.atom_b].entity_id);
            let local_a = (model.world_to_model * vec4f(endpoint_a, 1.0)).xyz;
            let local_b = (model.world_to_model * vec4f(endpoint_b, 1.0)).xyz;
            let distance = quality_capsule_distance(origin, direction, local_a, local_b, bond.radius);
            if distance > 0.0 && distance < local_maximum {
                let atom_a = atoms[bond.atom_a];
                let atom_b = atoms[bond.atom_b];
                let hit = origin + direction * distance;
                let axis = local_b - local_a;
                let along = clamp(dot(hit - local_a, axis) / max(dot(axis, axis), 1.0e-8), 0.0, 1.0);
                let nearest = local_a + axis * along;
                let entity_id = select(atom_a.entity_id, atom_b.entity_id, along >= 0.5);
                let base_color = mix(
                    atom_visual_color(atom_a.entity_id, atom_a.color),
                    atom_visual_color(atom_b.entity_id, atom_b.color),
                    along,
                );
                transmittance *= quality_hit_transparency(
                    entity_id,
                    base_color,
                    hit,
                    normalize(hit - nearest),
                );
                if transmittance <= MINIMUM_TRANSMITTANCE {
                    return 0.0;
                }
            }
        }
        node_index = escape;
    }
    return transmittance;
}
