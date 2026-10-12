/// Fraction of light that survives the ray.
fn trace_transmittance(world_origin: vec3f, world_direction: vec3f, maximum: f32) -> f32 {
    let origin = (model.world_to_model * vec4f(world_origin, 1.0)).xyz;
    let local_vector = (model.world_to_model * vec4f(world_direction, 0.0)).xyz;
    let local_scale = max(length(local_vector), 1e-6);
    let direction = local_vector / local_scale;
    let local_maximum = maximum * local_scale;
    let inverse_direction = 1.0 / direction;
    var transmittance = 1.0;
    let node_count = visual_counts.quality_bvh_nodes;
    let escape_base = quality_escape_base(visual_counts.quality_bvh_indices, node_count);
    var node_index = select(QUALITY_TRAVERSAL_END, 0u, node_count > 0u);
    while node_index != QUALITY_TRAVERSAL_END && node_index < node_count {
        let node = quality_node(node_index);
        let escape = quality_index(escape_base + node_index);
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
            let primitive_index = quality_index(first + offset);
            if primitive_index < visual_counts.atoms {
                let atom = atoms[primitive_index];
                let center = atom_local_position(atom_coordinate_base(atom.entity_id));
                let distance = quality_sphere_distance(origin, direction, center, atom.radius);
                if distance > 0.0 && distance < local_maximum {
                    let hit = origin + direction * distance;
                    transmittance *= quality_hit_transparency(atom.entity_id,
                        atom_record_color(atom), hit, normalize(hit - center));
                    if transmittance <= MINIMUM_TRANSMITTANCE { return 0.0; }
                }
                continue;
            }
            let bond = quality_bond(primitive_index - visual_counts.atoms);
            let local_a = atom_local_position(atom_coordinate_base(atoms[bond.atom_a].entity_id))
                + atom_visual_offset(atoms[bond.atom_a].entity_id);
            let local_b = atom_local_position(atom_coordinate_base(atoms[bond.atom_b].entity_id))
                + atom_visual_offset(atoms[bond.atom_b].entity_id);
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
                    atom_record_color(atom_a),
                    atom_record_color(atom_b),
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
