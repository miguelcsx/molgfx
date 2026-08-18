// Caller-supplied scalar-grid sampling with derivative-antialiased contours.
//
// Contracts for the enabled pipeline:
//   - overlay_size.xyz dimensions are >= 2.
//   - overlay_visual.z = 1 / (overlay_domain.y - overlay_domain.x).
//   - overlay_visual.w = 1 / (overlay_domain.z - overlay_domain.y).
//   - overlay_domain.w = inverse contour interval, or 0 to disable contours.
//
// SCALAR_OVERLAY_ENABLED allows the entire overlay path to be compiled out
// for representations that do not use scalar overlays.

override SCALAR_OVERLAY_ENABLED: bool = true;

const OVERLAY_NORMAL_EPSILON_SQ: f32 = 1.0e-12;
const OVERLAY_DERIVATIVE_EPSILON: f32 = 1.0e-6;
const OVERLAY_CONTOUR_DARKEN: f32 = 0.5616; // (1 - 0.22) * 0.72

@group(2) @binding(12)
var scalar_overlay_grid: texture_3d<f32>;

/// Applies an affine point transform without computing homogeneous W.
fn overlay_transform_point(
    transform: mat4x4f,
    point: vec3f,
) -> vec3f {
    return transform[0].xyz * point.x
        + transform[1].xyz * point.y
        + transform[2].xyz * point.z
        + transform[3].xyz;
}

/// Transforms a model-space normal into normalized world space.
fn overlay_world_normal(normal: vec3f) -> vec3f {
    let inverse_model =
        model.world_to_model;

    let world =
        vec3f(
            dot(inverse_model[0].xyz, normal),
            dot(inverse_model[1].xyz, normal),
            dot(inverse_model[2].xyz, normal),
        );

    return world *
        inverseSqrt(
            max(
                dot(world, world),
                OVERLAY_NORMAL_EPSILON_SQ,
            )
        );
}

/// Samples an already validated in-bounds coordinate with trilinear filtering.
///
/// Exactly eight texture loads are performed. Positive coordinates allow the
/// float-to-u32 conversion to replace floor().
fn overlay_sample(
    coordinate: vec3f,
    size: vec3u,
) -> f32 {
    let cell =
        min(
            vec3u(coordinate),
            size - vec3u(2u),
        );

    let fraction =
        coordinate -
        vec3f(cell);

    // Texel addressing is signed, so convert once rather than per corner:
    // vec3i has no mixed-component constructor to fall back on.
    let lower =
        vec3i(cell);

    let upper =
        lower + vec3i(1);

    let c00 =
        mix(
            textureLoad(
                scalar_overlay_grid,
                vec3i(lower),
                0,
            ).x,
            textureLoad(
                scalar_overlay_grid,
                vec3i(upper.x, lower.y, lower.z),
                0,
            ).x,
            fraction.x,
        );

    let c10 =
        mix(
            textureLoad(
                scalar_overlay_grid,
                vec3i(lower.x, upper.y, lower.z),
                0,
            ).x,
            textureLoad(
                scalar_overlay_grid,
                vec3i(upper.x, upper.y, lower.z),
                0,
            ).x,
            fraction.x,
        );

    let c01 =
        mix(
            textureLoad(
                scalar_overlay_grid,
                vec3i(lower.x, lower.y, upper.z),
                0,
            ).x,
            textureLoad(
                scalar_overlay_grid,
                vec3i(upper.x, lower.y, upper.z),
                0,
            ).x,
            fraction.x,
        );

    let c11 =
        mix(
            textureLoad(
                scalar_overlay_grid,
                vec3i(lower.x, upper.y, upper.z),
                0,
            ).x,
            textureLoad(
                scalar_overlay_grid,
                vec3i(upper),
                0,
            ).x,
            fraction.x,
        );

    return mix(
        mix(c00, c10, fraction.y),
        mix(c01, c11, fraction.y),
        fraction.z,
    );
}

/// Maps a scalar value through the caller-authored three-color ramp.
fn overlay_ramp(value: f32) -> vec3f {
    let domain =
        representation.overlay_domain.xyz;

    if value <= domain.y {
        let amount =
            clamp(
                (value - domain.x) *
                    representation.overlay_visual.z,
                0.0,
                1.0,
            );

        return mix(
            representation.overlay_colors[0].rgb,
            representation.overlay_colors[1].rgb,
            amount,
        );
    }

    let amount =
        clamp(
            (value - domain.y) *
                representation.overlay_visual.w,
            0.0,
            1.0,
        );

    return mix(
        representation.overlay_colors[1].rgb,
        representation.overlay_colors[2].rgb,
        amount,
    );
}

/// Applies the scalar overlay and optional derivative-antialiased contours.
fn scalar_overlay_color(
    local_position: vec3f,
    local_normal: vec3f,
    fallback: vec3f,
) -> vec3f {
    // Pipeline specialization removes the complete overlay path when false.
    if !SCALAR_OVERLAY_ENABLED {
        return fallback;
    }

    let world_position =
        overlay_transform_point(
            model.model_to_world,
            local_position,
        );

    let world_normal =
        overlay_world_normal(
            local_normal
        );

    let sample_position =
        fma(
            world_normal,
            vec3f(
                representation.overlay_visual.y
            ),
            world_position,
        );

    let coordinate =
        overlay_transform_point(
            representation.overlay_world_to_voxel,
            sample_position,
        );

    let size =
        representation.overlay_size.xyz;

    let extent =
        vec3f(
            size - vec3u(1u)
        );

    if any(coordinate < vec3f(0.0))
        || any(coordinate > extent) {
        return fallback;
    }

    let value =
        overlay_sample(
            coordinate,
            size,
        );

    var color =
        overlay_ramp(value);

    // W stores the reciprocal interval, so contours require no division.
    let inverse_interval =
        representation.overlay_domain.w;

    if inverse_interval <= 0.0 {
        return color;
    }

    let phase =
        abs(
            fract(
                (value -
                    representation.overlay_domain.y) *
                    inverse_interval +
                0.5
            ) -
            0.5
        );

    let width =
        max(
            fwidth(value) *
                inverse_interval,
            OVERLAY_DERIVATIVE_EPSILON,
        ) *
        representation.overlay_visual.x;

    let line =
        1.0 -
        smoothstep(
            width,
            width * 1.75,
            phase,
        );

    // Equivalent to:
    // mix(color, color * 0.22, line * 0.72)
    color *=
        1.0 -
        line *
        OVERLAY_CONTOUR_DARKEN;

    return color;
}
