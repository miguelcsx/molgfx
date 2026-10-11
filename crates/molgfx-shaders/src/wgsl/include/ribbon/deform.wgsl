// GPU-side spline, segment and residue deformation over resident coordinates.

struct RibbonDeformation {
    controls: vec4u,
    parameter: vec4f,
}

struct RibbonUniforms {
    planes: array<vec4f, 4>,
    metadata: vec4u,
    material: vec4f,
    presentation: vec4f,
    tube_mapping: vec4f,
    tube: vec4f,
}

struct DeformedRibbonVertex {
    position: vec3f,
    normal: vec3f,
    enabled: u32,
}

@group(2) @binding(4) var<storage, read> ribbon_coordinates: array<f32>;
@group(2) @binding(5) var<storage, read> ribbon_previous_coordinates: array<f32>;
@group(2) @binding(6) var<storage, read> ribbon_deformations: array<RibbonDeformation>;
@group(2) @binding(7) var<storage, read> ribbon_base_coordinates: array<f32>;
@group(2) @binding(13) var<storage, read> ribbon_radius_sources: array<f32>;
@group(2) @binding(3) var<uniform> ribbon_uniforms: RibbonUniforms;

fn ribbon_finite(value: f32) -> bool {
    return value == value && abs(value) <= 3.402823e38;
}

fn ribbon_mapped_radius(deformation: RibbonDeformation) -> f32 {
    let fallback = max(ribbon_uniforms.tube.x, 1e-6);
    if ribbon_uniforms.metadata.w == 0u {
        return fallback;
    }
    let indices = bitcast<vec2u>(deformation.parameter.yz);
    var left = 0.0;
    var right = 0.0;
    var left_valid = false;
    var right_valid = false;
    if indices.x != 0xffffffffu {
        left = ribbon_radius_sources[indices.x];
        left_valid = ribbon_finite(left);
    }
    if indices.y != 0xffffffffu {
        right = ribbon_radius_sources[indices.y];
        right_valid = ribbon_finite(right);
    }
    var value = 0.0;
    if left_valid && right_valid {
        value = left + (right - left) * deformation.parameter.x;
    } else if left_valid {
        value = left;
    } else if right_valid {
        value = right;
    } else {
        return fallback;
    }
    let domain = ribbon_uniforms.tube_mapping.xy;
    let radii = ribbon_uniforms.tube_mapping.zw;
    let amount = clamp((value - domain.x) / (domain.y - domain.x), 0.0, 1.0);
    return radii.x + amount * (radii.y - radii.x);
}

fn ribbon_coordinate(row: u32, previous: bool) -> vec3f {
    let base = row * 3u;
    if previous {
        return vec3f(
            ribbon_previous_coordinates[base],
            ribbon_previous_coordinates[base + 1u],
            ribbon_previous_coordinates[base + 2u],
        );
    }
    return vec3f(
        ribbon_coordinates[base],
        ribbon_coordinates[base + 1u],
        ribbon_coordinates[base + 2u],
    );
}

fn ribbon_base_coordinate(row: u32) -> vec3f {
    let base = row * 3u;
    return vec3f(
        ribbon_base_coordinates[base],
        ribbon_base_coordinates[base + 1u],
        ribbon_base_coordinates[base + 2u],
    );
}

fn ribbon_catmull_position(
    p0: vec3f,
    p1: vec3f,
    p2: vec3f,
    p3: vec3f,
    t: f32,
) -> vec3f {
    let t2 = t * t;
    let t3 = t2 * t;
    return (p1 * 2.0
        + (p2 - p0) * t
        + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
        + (-p0 + p1 * 3.0 - p2 * 3.0 + p3) * t3) * 0.5;
}

fn ribbon_catmull_tangent(
    p0: vec3f,
    p1: vec3f,
    p2: vec3f,
    p3: vec3f,
    t: f32,
) -> vec3f {
    let t2 = t * t;
    return ((p2 - p0)
        + (p0 * 4.0 - p1 * 10.0 + p2 * 8.0 - p3 * 2.0) * t
        + (-p0 * 3.0 + p1 * 9.0 - p2 * 9.0 + p3 * 3.0) * t2) * 0.5;
}

fn ribbon_rotation_arc(source: vec3f, destination: vec3f) -> vec4f {
    let cosine = clamp(dot(source, destination), -1.0, 1.0);
    if cosine < -0.999999 {
        let absolute = abs(source);
        var seed = vec3f(0.0, 0.0, 1.0);
        if absolute.x <= absolute.y && absolute.x <= absolute.z {
            seed = vec3f(1.0, 0.0, 0.0);
        } else if absolute.y <= absolute.z {
            seed = vec3f(0.0, 1.0, 0.0);
        }
        return vec4f(normalize(cross(source, seed)), 0.0);
    }
    return normalize(vec4f(cross(source, destination), 1.0 + cosine));
}

fn ribbon_rotate(rotation: vec4f, value: vec3f) -> vec3f {
    let twice = 2.0 * cross(rotation.xyz, value);
    return value + rotation.w * twice + cross(rotation.xyz, twice);
}

const RIBBON_ATOM_ANCHOR: u32 = 1u;
const RIBBON_GAP_ANCHOR: u32 = 2u;
const RIBBON_LINEAR_ANCHOR: u32 = 3u;
const RIBBON_PEPTIDE_ANCHOR: u32 = 4u;
const RIBBON_RESIDUE_ANCHOR: u32 = 5u;
const RIBBON_HELIX_ANCHOR: u32 = 6u;

fn ribbon_fit_vector(offset: u32) -> vec3f {
    return vec3f(ribbon_radius_sources[offset], ribbon_radius_sources[offset+1u], ribbon_radius_sources[offset+2u]);
}

fn ribbon_residue_basis(a: vec3f, b: vec3f, c: vec3f) -> mat3x3f {
    let x = normalize(b - a);
    let z = normalize(cross(x, c - a));
    return mat3x3f(x, cross(z, x), z);
}

fn ribbon_peptide_width(left: vec3f, right: vec3f, tangent: vec3f, amount: f32) -> vec3f {
    let first = left - tangent * dot(left, tangent);
    let second = right - tangent * dot(right, tangent);
    if dot(first, first) < 1e-12 || dot(second, second) < 1e-12 { return vec3f(0.0); }
    let a = normalize(first);
    let b = normalize(second) * select(1.0, -1.0, dot(first, second) < 0.0);
    return normalize(mix(a, b, amount));
}

fn ribbon_gap_vertex(
    deformation: RibbonDeformation,
    base_position: vec3f,
    base_normal: vec3f,
    start: vec3f,
    end: vec3f,
    reference_start: vec3f,
    reference_end: vec3f,
) -> DeformedRibbonVertex {
    let span = end - start;
    let span_length = length(span);
    // Collapse reserved dashes past the current endpoint to zero-area triangles.
    if span_length <= deformation.parameter.y {
        return DeformedRibbonVertex(start, base_normal, 1u);
    }
    let tangent = span / span_length;
    let reference_tangent = normalize(reference_end - reference_start);
    let offset = deformation.parameter.x;
    let reference_center = reference_start + reference_tangent * offset;
    let center = start + tangent * min(offset, span_length);
    let rotation = ribbon_rotation_arc(reference_tangent, tangent);
    return DeformedRibbonVertex(
        center + ribbon_rotate(rotation, base_position - reference_center),
        normalize(ribbon_rotate(rotation, base_normal)),
        1u,
    );
}

fn ribbon_deform(
    vertex_id: u32,
    base_position: vec3f,
    base_normal: vec3f,
    previous: bool,
) -> DeformedRibbonVertex {
    if ribbon_uniforms.tube.w == 0.0 {
        return DeformedRibbonVertex(vec3f(0.0), vec3f(0.0), 0u);
    }
    let deformation = ribbon_deformations[vertex_id];
    if deformation.controls.x == 0xffffffffu {
        return DeformedRibbonVertex(vec3f(0.0), vec3f(0.0), 0u);
    }
    let p0 = ribbon_coordinate(deformation.controls.x, previous);
    let p1 = ribbon_coordinate(deformation.controls.y, previous);
    let p2 = ribbon_coordinate(deformation.controls.z, previous);
    let p3 = ribbon_coordinate(deformation.controls.w, previous);
    let r0 = ribbon_base_coordinate(deformation.controls.x);
    let r1 = ribbon_base_coordinate(deformation.controls.y);
    let r2 = ribbon_base_coordinate(deformation.controls.z);
    let r3 = ribbon_base_coordinate(deformation.controls.w);
    if bitcast<u32>(deformation.parameter.w) == RIBBON_RESIDUE_ANCHOR {
        let reference_cross = cross(r1 - r0, r2 - r0);
        let current_cross = cross(p1 - p0, p2 - p0);
        if dot(reference_cross, reference_cross) <= 1e-12 || dot(current_cross, current_cross) <= 1e-12 {
            return DeformedRibbonVertex(base_position + p0 - r0, base_normal, 1u);
        }
        let current_normal = normalize(current_cross);
        let reference_normal = normalize(reference_cross);
        let local = deformation.parameter.xyz;
        let position = p0 + local.x * (p1 - p0) + local.y * (p2 - p0) + local.z * current_normal;
        // Transform normals with the inverse transpose of the local affine map.
        let normal = cross(p2 - p0, current_normal) * dot(base_normal, r1 - r0)
            + cross(current_normal, p1 - p0) * dot(base_normal, r2 - r0)
            + current_cross * dot(base_normal, reference_normal);
        return DeformedRibbonVertex(position, normalize(normal), 1u);
    }
    if bitcast<u32>(deformation.parameter.w) == RIBBON_GAP_ANCHOR {
        return ribbon_gap_vertex(deformation, base_position, base_normal, p0, p1, r0, r1);
    }
    let parameter = deformation.parameter.x;
    var center = ribbon_catmull_position(p0, p1, p2, p3, parameter);
    var derivative = ribbon_catmull_tangent(p0, p1, p2, p3, parameter);
    var reference_center = ribbon_catmull_position(r0, r1, r2, r3, parameter);
    var reference_derivative = ribbon_catmull_tangent(r0, r1, r2, r3, parameter);
    if bitcast<u32>(deformation.parameter.w) == RIBBON_ATOM_ANCHOR {
        center = p1;
        derivative = p2 - p0;
        reference_center = r1;
        reference_derivative = r2 - r0;
    }
    if bitcast<u32>(deformation.parameter.w) == RIBBON_LINEAR_ANCHOR {
        center = mix(p1, p2, parameter);
        derivative = p2 - p1;
        reference_center = mix(r1, r2, parameter);
        reference_derivative = r2 - r1;
    }
    if bitcast<u32>(deformation.parameter.w) == RIBBON_HELIX_ANCHOR {
        let fit = bitcast<u32>(deformation.parameter.y);
        let current_fit = fit + select(0u, 8u, previous);
        let origin = ribbon_fit_vector(current_fit);
        let axis = ribbon_fit_vector(current_fit + 4u);
        let reference_origin = ribbon_fit_vector(fit + 16u);
        let reference_axis = ribbon_fit_vector(fit + 20u);
        let projected = mix(p0, p1, parameter);
        let reference_projected = mix(r0, r1, parameter);
        center = origin + axis * dot(projected - origin, axis);
        derivative = axis;
        reference_center = reference_origin + reference_axis * dot(reference_projected - reference_origin, reference_axis);
        reference_derivative = reference_axis;
    }
    let length_sq = dot(derivative, derivative);
    let reference_length_sq = dot(reference_derivative, reference_derivative);
    let reference_tangent = select(
        vec3f(0.0, 0.0, 1.0),
        reference_derivative * inverseSqrt(reference_length_sq),
        reference_length_sq > 1e-12,
    );
    let tangent = select(reference_tangent, derivative * inverseSqrt(length_sq), length_sq > 1e-12);
    var rotation = ribbon_rotation_arc(reference_tangent, tangent);
    if bitcast<u32>(deformation.parameter.w) == RIBBON_PEPTIDE_ANCHOR {
        let rows = bitcast<vec2u>(deformation.parameter.yz);
        let width = ribbon_peptide_width(
            ribbon_coordinate(rows.x, previous) - p1,
            ribbon_coordinate(rows.y, previous) - p2, tangent, parameter,
        );
        let base_width = ribbon_peptide_width(
            ribbon_base_coordinate(rows.x) - r1,
            ribbon_base_coordinate(rows.y) - r2, reference_tangent, parameter,
        );
        let transported = ribbon_rotate(rotation, base_width);
        if dot(width, width) > 0.0 && dot(base_width, base_width) > 0.0 {
            let oriented_width = width * select(1.0, -1.0, dot(width, transported) < 0.0);
            let twist = ribbon_rotation_arc(transported, oriented_width);
            // Compose the twist after the tangent rotation.
            rotation = vec4f(
                twist.w * rotation.xyz + rotation.w * twist.xyz + cross(twist.xyz, rotation.xyz),
                twist.w * rotation.w - dot(twist.xyz, rotation.xyz),
            );
        }
    }
    var radius_scale = 1.0;
    if bitcast<u32>(deformation.parameter.w) != RIBBON_HELIX_ANCHOR {
        radius_scale = ribbon_mapped_radius(deformation) / max(ribbon_uniforms.tube.x, 1e-6);
    }
    let position = center
        + ribbon_rotate(rotation, base_position - reference_center) * radius_scale;
    let surface_normal = normalize(ribbon_rotate(rotation, base_normal));
    return DeformedRibbonVertex(position, surface_normal, 1u);
}
