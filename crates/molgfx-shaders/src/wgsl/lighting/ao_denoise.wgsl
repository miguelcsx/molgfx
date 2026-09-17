// Edge-aware denoise of traced occlusion and shadow terms.
//
// Exact 5x5 bilateral kernel with the center handled separately.
// Spatial Gaussian weights are compile-time constants: no exp() is evaluated
// per fragment. Normal^24 uses fixed multiplications instead of generic pow().
//
// Contract:
//   - gbuffer textures match frame.viewport dimensions.

//!include "include/camera.wgsl"
//!include "include/fullscreen.wgsl"
//!include "include/surface_frame.wgsl"

@group(1) @binding(0) var occlusion_texture: texture_2d<f32>;
@group(1) @binding(1) var depth_texture: texture_depth_2d;
@group(1) @binding(2) var normal_texture: texture_2d<f32>;

const DEPTH_TOLERANCE: f32 = 0.035;
const DEPTH_EPSILON: f32 = 1.0e-4;
const VIEW_W_EPSILON: f32 = 1.0e-7;

const DENOISE_OFFSETS: array<vec2i, 24> = array<vec2i, 24>(
    vec2i(-2, -2),
    vec2i(-1, -2),
    vec2i( 0, -2),
    vec2i( 1, -2),
    vec2i( 2, -2),

    vec2i(-2, -1),
    vec2i(-1, -1),
    vec2i( 0, -1),
    vec2i( 1, -1),
    vec2i( 2, -1),

    vec2i(-2,  0),
    vec2i(-1,  0),
    vec2i( 1,  0),
    vec2i( 2,  0),

    vec2i(-2,  1),
    vec2i(-1,  1),
    vec2i( 0,  1),
    vec2i( 1,  1),
    vec2i( 2,  1),

    vec2i(-2,  2),
    vec2i(-1,  2),
    vec2i( 0,  2),
    vec2i( 1,  2),
    vec2i( 2,  2),
);

// exp(-distance² / 8), equivalent to sigma = 2.
const DENOISE_SPATIAL: array<f32, 24> = array<f32, 24>(
    0.36787944,
    0.53526143,
    0.60653066,
    0.53526143,
    0.36787944,

    0.53526143,
    0.77880078,
    0.88249690,
    0.77880078,
    0.53526143,

    0.60653066,
    0.88249690,
    0.88249690,
    0.60653066,

    0.53526143,
    0.77880078,
    0.88249690,
    0.77880078,
    0.53526143,

    0.36787944,
    0.53526143,
    0.60653066,
    0.53526143,
    0.36787944,
);

/// Converts a pixel center to NDC using cached inverse viewport dimensions.
fn denoise_pixel_ndc(pixel: vec2i) -> vec2f {
    return fma(
        vec2f(pixel) + vec2f(0.5),
        frame.viewport.zw * vec2f(2.0, -2.0),
        vec2f(-1.0, 1.0),
    );
}

/// Reconstructs only absolute view-space depth.
///
/// Full view-space XYZ is unnecessary for bilateral depth rejection.
fn denoise_view_depth(
    pixel: vec2i,
    raw_depth: f32,
) -> f32 {
    let ndc =
        denoise_pixel_ndc(pixel);

    let zw =
        frame.inv_proj[0].zw * ndc.x
        + frame.inv_proj[1].zw * ndc.y
        + frame.inv_proj[2].zw * raw_depth
        + frame.inv_proj[3].zw;

    return abs(zw.x) /
        max(
            abs(zw.y),
            VIEW_W_EPSILON,
        );
}

/// Computes alignment^24 using five multiplies instead of generic pow().
fn denoise_normal_weight(alignment: f32) -> f32 {
    let x2 =
        alignment * alignment;

    let x4 =
        x2 * x2;

    let x8 =
        x4 * x4;

    let x16 =
        x8 * x8;

    return x16 * x8;
}

@fragment
fn fs_denoise_occlusion(
    in: FullscreenOut,
) -> @location(0) vec4f {
    let dimensions =
        vec2i(frame.viewport.xy);

    let pixel =
        vec2i(in.position.xy);

    let center =
        textureLoad(
            occlusion_texture,
            pixel,
            0,
        );

    let center_raw_depth =
        textureLoad(
            depth_texture,
            pixel,
            0,
        );

    if center_raw_depth <= 0.0 {
        return center;
    }

    let center_depth =
        denoise_view_depth(
            pixel,
            center_raw_depth,
        );

    let center_normal =
        decode_shading_frame(
            textureLoad(
                normal_texture,
                pixel,
                0,
            ).xyz
        ).normal;

    let inverse_depth_tolerance =
        1.0 /
        max(
            center_depth *
                DEPTH_TOLERANCE,
            DEPTH_EPSILON,
        );

    let minimum_pixel =
        vec2i(0);

    let maximum_pixel =
        dimensions - 1;

    // Most fragments are interior and require no per-tap coordinate clamp.
    let interior =
        all(pixel >= vec2i(2))
        && all(
            pixel <
            dimensions - vec2i(2)
        );

    var total =
        center.rg;

    var total_weight =
        1.0;

    for (
        var index = 0u;
        index < 24u;
        index++
    ) {
        var sample_pixel =
            pixel +
            DENOISE_OFFSETS[index];

        if !interior {
            sample_pixel =
                clamp(
                    sample_pixel,
                    minimum_pixel,
                    maximum_pixel,
                );
        }

        let sample_raw_depth =
            textureLoad(
                depth_texture,
                sample_pixel,
                0,
            );

        if sample_raw_depth <= 0.0 {
            continue;
        }

        let sample_depth =
            denoise_view_depth(
                sample_pixel,
                sample_raw_depth,
            );

        let depth_difference =
            abs(
                sample_depth -
                center_depth
            ) * inverse_depth_tolerance;

        if depth_difference > 1.0 {
            continue;
        }

        let sample_normal =
            decode_shading_frame(
                textureLoad(
                    normal_texture,
                    sample_pixel,
                    0,
                ).xyz
            ).normal;

        let alignment =
            max(
                dot(
                    center_normal,
                    sample_normal,
                ),
                0.0,
            );

        if alignment <= 0.0 {
            continue;
        }

        let depth_weight =
            fma(
                -depth_difference,
                depth_difference,
                1.0,
            );

        let weight =
            DENOISE_SPATIAL[index]
            * denoise_normal_weight(
                alignment
            )
            * depth_weight;

        if weight <= 0.0 {
            continue;
        }

        total +=
            textureLoad(
                occlusion_texture,
                sample_pixel,
                0,
            ).rg *
            weight;

        total_weight +=
            weight;
    }

    return vec4f(
        total / total_weight,
        center.ba,
    );
}
