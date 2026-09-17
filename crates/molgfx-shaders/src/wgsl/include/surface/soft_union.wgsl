// Small fixed-width soft union used only by the explicit SoftUnion style.

fn soft_min_parameter(left: f32, right: f32, span: f32) -> f32 {
    let blend = max(span - abs(left - right), 0.0);
    return min(left, right) - blend * blend / (4.0 * span);
}

fn soft_union_parameter(parameters: array<f32, 4>, span: f32) -> f32 {
    var resolved = parameters[0];
    for (var index = 1u; index < 4u; index++) {
        if parameters[index] == SURFACE_INFINITY {
            break;
        }
        resolved = soft_min_parameter(resolved, parameters[index], span);
    }
    return resolved;
}

fn union_atom_surface_normal(
    point: vec3f,
    compact_index: u32,
    inflation: f32,
) -> vec3f {
    let atom = atoms[compact_index];
    let source_index = atom.entity_id & BVH_INDEX_MASK;
    let base = source_index * 3u;
    let center = vec3f(coords[base], coords[base + 1u], coords[base + 2u]);
    let radius = max(
        atom.radius * representation.visual.w + inflation,
        SURFACE_RAY_EPSILON,
    );
    return (point - center) / radius;
}

fn soft_union_surface_normal(
    point: vec3f,
    parameters: array<f32, 4>,
    indices: array<u32, 4>,
    span: f32,
    inflation: f32,
) -> vec3f {
    var normal = union_atom_surface_normal(point, indices[0], inflation);
    var total = 1.0;
    for (var index = 1u; index < 4u; index++) {
        if indices[index] == EMPTY_COMPACT_INDEX {
            break;
        }
        let weight = 1.0 - smoothstep(
            0.0,
            span,
            parameters[index] - parameters[0],
        );
        normal += union_atom_surface_normal(point, indices[index], inflation) * weight;
        total += weight;
    }
    return normalize(normal / total);
}
