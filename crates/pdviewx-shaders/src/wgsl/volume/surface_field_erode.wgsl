// Erodes the probe-inflated field into the rolling-probe SES.
//
// Erosion offsets are caller-precomputed and already scaled by probe radius.
// Dispatch is native 3D: one invocation per grid voxel.

//!include "include/atom.wgsl"
//!include "include/surface_field.wgsl"

@group(1) @binding(0)
var inflated_field: texture_3d<f32>;

@group(1) @binding(1)
var output_field:
    texture_storage_3d<r32float, write>;

// xyz = precomputed direction * representation.surface.x.
@group(1) @binding(2)
var<storage, read>
erosion_offsets: array<vec4f>;

/// Trilinear sample of the probe-inflated field.
fn sample_inflated(
    point: vec3f,
    inverse_cell: vec3f,
) -> f32 {
    let size =
        representation.grid_size.xyz;

    let extent =
        vec3f(
            size -
            vec3u(1u)
        );

    let coordinate =
        clamp(
            (
                point -
                representation.grid_min.xyz
            ) * inverse_cell,
            vec3f(0.0),
            extent,
        );

    let cell =
        min(
            vec3u(coordinate),
            size - vec3u(2u),
        );

    let fraction =
        coordinate -
        vec3f(cell);

    // Texel addressing is signed and vec3i has no mixed-component
    // constructor, so convert once rather than at each corner below.
    let lower =
        vec3i(cell);

    let upper =
        lower +
        vec3i(1);

    let c00 =
        mix(
            textureLoad(
                inflated_field,
                vec3i(lower),
                0,
            ).x,
            textureLoad(
                inflated_field,
                vec3i(
                    upper.x,
                    lower.y,
                    lower.z
                ),
                0,
            ).x,
            fraction.x,
        );

    let c10 =
        mix(
            textureLoad(
                inflated_field,
                vec3i(
                    lower.x,
                    upper.y,
                    lower.z
                ),
                0,
            ).x,
            textureLoad(
                inflated_field,
                vec3i(
                    upper.x,
                    upper.y,
                    lower.z
                ),
                0,
            ).x,
            fraction.x,
        );

    let c01 =
        mix(
            textureLoad(
                inflated_field,
                vec3i(
                    lower.x,
                    lower.y,
                    upper.z
                ),
                0,
            ).x,
            textureLoad(
                inflated_field,
                vec3i(
                    upper.x,
                    lower.y,
                    upper.z
                ),
                0,
            ).x,
            fraction.x,
        );

    let c11 =
        mix(
            textureLoad(
                inflated_field,
                vec3i(
                    lower.x,
                    upper.y,
                    upper.z
                ),
                0,
            ).x,
            textureLoad(
                inflated_field,
                vec3i(upper),
                0,
            ).x,
            fraction.x,
        );

    return mix(
        mix(
            c00,
            c10,
            fraction.y,
        ),
        mix(
            c01,
            c11,
            fraction.y,
        ),
        fraction.z,
    );
}

@compute @workgroup_size(4, 4, 4)
fn cs_surface_field_erode(
    @builtin(global_invocation_id)
    coordinate: vec3u,
) {
    let size =
        representation.grid_size.xyz;

    if any(coordinate >= size) {
        return;
    }

    let texel =
        vec3i(coordinate);

    let point =
        representation.grid_min.xyz +
        vec3f(coordinate) *
        representation.grid_cell.xyz;

    let inverse_cell =
        vec3f(1.0) /
        representation.grid_cell.xyz;

    var field =
        textureLoad(
            inflated_field,
            texel,
            0,
        ).x;

    let sample_count =
        min(
            representation.options.z,
            arrayLength(
                &erosion_offsets
            ),
        );

    for (
        var index = 0u;
        index < sample_count;
        index++
    ) {
        let sample =
            sample_inflated(
                point +
                erosion_offsets[index].xyz,
                inverse_cell,
            );

        if sample <= field {
            continue;
        }

        field =
            sample;
    }

    textureStore(
        output_field,
        texel,
        vec4f(
            field,
            0.0,
            0.0,
            0.0,
        ),
    );
}
