// GPU-side Catmull-Rom ribbon deformation over resident coordinate columns.

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

fn ribbon_deform(
    vertex_id: u32,
    base_position: vec3f,
    base_normal: vec3f,
    previous: bool,
) -> DeformedRibbonVertex {
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
    let parameter = deformation.parameter.x;
    let center = ribbon_catmull_position(p0, p1, p2, p3, parameter);
    let derivative = ribbon_catmull_tangent(p0, p1, p2, p3, parameter);
    let reference_center = ribbon_catmull_position(r0, r1, r2, r3, parameter);
    let reference_derivative = ribbon_catmull_tangent(r0, r1, r2, r3, parameter);
    let length_sq = dot(derivative, derivative);
    let reference_length_sq = dot(reference_derivative, reference_derivative);
    let reference_tangent = select(
        vec3f(0.0, 0.0, 1.0),
        reference_derivative * inverseSqrt(reference_length_sq),
        reference_length_sq > 1e-12,
    );
    let tangent = select(reference_tangent, derivative * inverseSqrt(length_sq), length_sq > 1e-12);
    let rotation = ribbon_rotation_arc(reference_tangent, tangent);
    let radius_scale = ribbon_mapped_radius(deformation) / max(ribbon_uniforms.tube.x, 1e-6);
    let position = center
        + ribbon_rotate(rotation, base_position - reference_center) * radius_scale;
    let surface_normal = normalize(ribbon_rotate(rotation, base_normal));
    return DeformedRibbonVertex(position, surface_normal, 1u);
}
