// Compact view-space shading frames. The gbuffer stores an octahedral normal
// and one rotation about that normal, preserving its existing four half-floats.

const SURFACE_FRAME_PI: f32 = 3.14159265;

struct ShadingFrame {
    normal: vec3f,
    tangent: vec3f,
}

fn sign_not_zero(value: vec2f) -> vec2f {
    return select(vec2f(-1.0), vec2f(1.0), value >= vec2f(0.0));
}

fn encode_octahedral(normal: vec3f) -> vec2f {
    let unit = normal / max(abs(normal.x) + abs(normal.y) + abs(normal.z), 1e-7);
    if unit.z < 0.0 {
        return (vec2f(1.0) - abs(unit.yx)) * sign_not_zero(unit.xy);
    }
    return unit.xy;
}

fn decode_octahedral(encoded: vec2f) -> vec3f {
    var normal = vec3f(encoded, 1.0 - abs(encoded.x) - abs(encoded.y));
    let fold = clamp(-normal.z, 0.0, 1.0);
    normal.x += select(fold, -fold, normal.x >= 0.0);
    normal.y += select(fold, -fold, normal.y >= 0.0);
    return normalize(normal);
}

fn canonical_tangent(normal: vec3f) -> vec3f {
    let axis = select(vec3f(0.0, 0.0, 1.0), vec3f(0.0, 1.0, 0.0), abs(normal.z) > 0.9);
    return normalize(cross(axis, normal));
}

fn encode_shading_frame(normal_value: vec3f, tangent_value: vec3f) -> vec3f {
    let normal = normalize(normal_value);
    let basis_tangent = canonical_tangent(normal);
    let basis_bitangent = cross(normal, basis_tangent);
    let projected = tangent_value - normal * dot(normal, tangent_value);
    let length_squared = dot(projected, projected);
    let tangent = select(
        basis_tangent,
        projected * inverseSqrt(max(length_squared, 1e-8)),
        length_squared > 1e-8,
    );
    let angle = atan2(dot(tangent, basis_bitangent), dot(tangent, basis_tangent));
    return vec3f(encode_octahedral(normal), angle / SURFACE_FRAME_PI);
}

fn decode_shading_frame(encoded: vec3f) -> ShadingFrame {
    let normal = decode_octahedral(encoded.xy);
    let basis_tangent = canonical_tangent(normal);
    let basis_bitangent = cross(normal, basis_tangent);
    let angle = clamp(encoded.z, -1.0, 1.0) * SURFACE_FRAME_PI;
    return ShadingFrame(
        normal,
        normalize(basis_tangent * cos(angle) + basis_bitangent * sin(angle)),
    );
}

fn curve_tangent(view_position: vec3f, curve_parameter: f32, normal: vec3f) -> vec3f {
    let derivative = dpdx(view_position) * dpdx(curve_parameter)
        + dpdy(view_position) * dpdy(curve_parameter);
    let projected = derivative - normal * dot(normal, derivative);
    let length_squared = dot(projected, projected);
    return select(
        canonical_tangent(normal),
        projected * inverseSqrt(max(length_squared, 1e-8)),
        length_squared > 1e-8,
    );
}
