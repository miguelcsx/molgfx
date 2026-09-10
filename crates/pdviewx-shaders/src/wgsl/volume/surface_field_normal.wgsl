// Builds one compact, continuously interpolated normal field after the final
// molecular scalar field has been generated.
//
// Central differences run only when the persistent field changes. Shading then
// reads one eight-texel cell, keeping per-fragment work constant while avoiding
// the discontinuous derivative of per-cell trilinear scalar interpolation.

//!include "include/representation.wgsl"

@group(1) @binding(0)
var input_field: texture_3d<f32>;

@group(1) @binding(1)
var output_normals:
    texture_storage_3d<rgba8snorm, write>;

fn field_value(coordinate: vec3u) -> f32 {
    return textureLoad(
        input_field,
        vec3i(coordinate),
        0,
    ).x;
}

@compute @workgroup_size(4, 4, 4)
fn cs_surface_field_normal(
    @builtin(global_invocation_id)
    coordinate: vec3u,
) {
    let size = representation.grid_size.xyz;

    if any(coordinate >= size) {
        return;
    }

    let lower = max(coordinate, vec3u(1u)) - vec3u(1u);
    let upper = min(coordinate + vec3u(1u), size - vec3u(1u));
    let span = max(
        vec3f(upper - lower) * representation.grid_cell.xyz,
        vec3f(1.0e-6),
    );

    var gradient = vec3f(
        field_value(vec3u(upper.x, coordinate.y, coordinate.z)) -
            field_value(vec3u(lower.x, coordinate.y, coordinate.z)),
        field_value(vec3u(coordinate.x, upper.y, coordinate.z)) -
            field_value(vec3u(coordinate.x, lower.y, coordinate.z)),
        field_value(vec3u(coordinate.x, coordinate.y, upper.z)) -
            field_value(vec3u(coordinate.x, coordinate.y, lower.z)),
    ) / span;

    if representation.options.x == SURFACE_KIND_GAUSSIAN {
        gradient = -gradient;
    }

    let magnitude_sq = dot(gradient, gradient);
    let normal = select(
        vec3f(0.0, 0.0, 1.0),
        gradient * inverseSqrt(magnitude_sq),
        magnitude_sq >= 1.0e-10,
    );

    textureStore(
        output_normals,
        vec3i(coordinate),
        vec4f(normal, 1.0),
    );
}
