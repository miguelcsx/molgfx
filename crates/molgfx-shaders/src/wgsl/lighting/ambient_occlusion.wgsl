// Deterministic molecular SSAO over the analytic depth/normal gbuffer.
//
// Twelve fixed taps provide stable cavity/contact response without temporal
// noise. View reconstruction uses the frame's cached inverse viewport.
//
// Contact shadow projection is incremental: the first ray point and its step
// are projected once, then advanced linearly in clip XYW.
//
// Contract:
//   depth_texture and normal_texture match frame.viewport dimensions.

//!include "include/camera.wgsl"
//!include "include/fullscreen.wgsl"
//!include "include/surface_frame.wgsl"

@group(1) @binding(0) var depth_texture: texture_depth_2d;
@group(1) @binding(1) var normal_texture: texture_2d<f32>;

const VIEW_W_EPSILON: f32 = 1.0e-7;

const AO_BIAS: f32 = 0.08;
const AO_BIAS_SQ: f32 = AO_BIAS * AO_BIAS;
const AO_MIN_DISTANCE_SQ: f32 = 0.16; // 0.4²
const AO_MAX_DISTANCE_SQ: f32 = 36.0; // 6²
const AO_DISTANCE_EPSILON_SQ: f32 = 1.0e-10;
const AO_STRENGTH: f32 = 0.42;
const AO_MIN_VISIBILITY: f32 = 0.18;

const CONTACT_STEPS: u32 = 6u;
const CONTACT_ORIGIN_OFFSET: f32 = 0.22;
const CONTACT_STEP_LENGTH: f32 = 0.72;
const CONTACT_MIN_SEPARATION: f32 = 0.10;
const CONTACT_MAX_SEPARATION: f32 = 1.8;

const AO_OFFSETS: array<vec2i, 12> = array<vec2i, 12>(
    vec2i(2, 0),
    vec2i(-2, 0),
    vec2i(0, 2),
    vec2i(0, -2),
    vec2i(3, 3),
    vec2i(-3, 3),
    vec2i(3, -3),
    vec2i(-3, -3),
    vec2i(7, 2),
    vec2i(-7, 2),
    vec2i(2, 7),
    vec2i(2, -7),
);

/// Converts a pixel center directly to NDC.
fn pixel_ndc(pixel: vec2i) -> vec2f {
    return fma(
        vec2f(pixel) + vec2f(0.5),
        frame.viewport.zw * vec2f(2.0, -2.0),
        vec2f(-1.0, 1.0),
    );
}

/// Returns the safe reciprocal homogeneous W.
fn homogeneous_inverse(w: f32) -> f32 {
    return sign(w) /
        max(
            abs(w),
            VIEW_W_EPSILON,
        );
}

/// Reconstructs a complete view-space position.
fn reconstruct_view_position(
    pixel: vec2i,
    depth: f32,
) -> vec3f {
    let ndc =
        pixel_ndc(pixel);

    let view =
        frame.inv_proj[0] * ndc.x
        + frame.inv_proj[1] * ndc.y
        + frame.inv_proj[2] * depth
        + frame.inv_proj[3];

    return view.xyz *
        homogeneous_inverse(view.w);
}

/// Reconstructs only view-space Z.
///
/// Contact shadows never consume reconstructed X/Y, so this avoids most of
/// the inverse-projection result.
fn reconstruct_view_z(
    pixel: vec2i,
    depth: f32,
) -> f32 {
    let ndc =
        pixel_ndc(pixel);

    let zw =
        frame.inv_proj[0].zw * ndc.x
        + frame.inv_proj[1].zw * ndc.y
        + frame.inv_proj[2].zw * depth
        + frame.inv_proj[3].zw;

    return zw.x *
        homogeneous_inverse(zw.y);
}

/// Projects a view-space point to clip X/Y/W only.
fn project_view_xyw(point: vec3f) -> vec3f {
    return frame.proj[0].xyw * point.x
        + frame.proj[1].xyw * point.y
        + frame.proj[2].xyw * point.z
        + frame.proj[3].xyw;
}

/// Projects a view-space direction to clip X/Y/W without translation.
fn project_view_direction_xyw(
    direction: vec3f,
) -> vec3f {
    return frame.proj[0].xyw * direction.x
        + frame.proj[1].xyw * direction.y
        + frame.proj[2].xyw * direction.z;
}

/// Resolves deterministic screen-space contact shadowing along the key light.
fn contact_shadow(
    center: vec3f,
    normal: vec3f,
    dimensions: vec2i,
) -> f32 {
    let shadow_strength =
        frame.lighting[3].w;

    if shadow_strength <= 0.0 {
        return 1.0;
    }

    let light =
        frame.lighting[4].xyz;

    let light_length_sq =
        dot(light, light);

    if light_length_sq <=
        AO_DISTANCE_EPSILON_SQ {
        return 1.0;
    }

    let ray_step =
        light *
        (
            CONTACT_STEP_LENGTH *
            inverseSqrt(light_length_sq)
        );

    let first =
        center
        + normal * CONTACT_ORIGIN_OFFSET
        + ray_step;

    // Projection is linear along the ray: project only the first point and
    // the constant increment rather than six independent matrix products.
    var clip =
        project_view_xyw(first);

    let clip_step =
        project_view_direction_xyw(
            ray_step
        );

    var ray_z =
        first.z;

    for (
        var step = 0u;
        step < CONTACT_STEPS;
        step++
    ) {
        if clip.z > VIEW_W_EPSILON {
            let ndc =
                clip.xy *
                (1.0 / clip.z);

            let uv =
                ndc * vec2f(0.5, -0.5)
                + vec2f(0.5);

            if all(uv >= vec2f(0.0))
                && all(uv <= vec2f(1.0)) {
                let sample_pixel =
                    clamp(
                        vec2i(
                            uv *
                            vec2f(dimensions)
                        ),
                        vec2i(0),
                        dimensions - 1,
                    );

                let sample_depth =
                    textureLoad(
                        depth_texture,
                        sample_pixel,
                        0,
                    );

                if sample_depth > 0.0 {
                    let surface_z =
                        reconstruct_view_z(
                            sample_pixel,
                            sample_depth,
                        );

                    let separation =
                        surface_z - ray_z;

                    if separation >
                            CONTACT_MIN_SEPARATION
                        && separation <
                            CONTACT_MAX_SEPARATION {
                        return 1.0 -
                            shadow_strength;
                    }
                }
            }
        }

        clip += clip_step;
        ray_z += ray_step.z;
    }

    return 1.0;
}

/// Evaluates one AO tap, rejecting irrelevant samples before normalization.
fn ambient_occlusion_sample(
    center: vec3f,
    normal: vec3f,
    pixel: vec2i,
) -> f32 {
    let depth =
        textureLoad(
            depth_texture,
            pixel,
            0,
        );

    if depth <= 0.0 {
        return 0.0;
    }

    let delta =
        reconstruct_view_position(
            pixel,
            depth,
        ) - center;

    let distance_sq =
        dot(delta, delta);

    // smoothstep() makes every sample at >= 6 view units contribute zero.
    if distance_sq >=
        AO_MAX_DISTANCE_SQ {
        return 0.0;
    }

    let projected =
        dot(
            normal,
            delta,
        );

    if projected <= 0.0 {
        return 0.0;
    }

    let safe_distance_sq =
        max(
            distance_sq,
            AO_DISTANCE_EPSILON_SQ,
        );

    // Equivalent rejection to:
    // dot(normal, normalize(delta)) <= AO_BIAS
    //
    // but avoids sqrt/inverseSqrt for non-contributing taps.
    if projected * projected <=
        AO_BIAS_SQ *
        safe_distance_sq {
        return 0.0;
    }

    let inverse_distance =
        inverseSqrt(
            safe_distance_sq
        );

    let horizon =
        projected *
        inverse_distance -
        AO_BIAS;

    // distance = sqrt(distance_sq), reusing the reciprocal square root.
    let distance =
        distance_sq *
        inverse_distance;

    var range = 1.0;

    if distance_sq >
        AO_MIN_DISTANCE_SQ {
        range =
            1.0 -
            smoothstep(
                0.4,
                6.0,
                distance,
            );
    }

    return horizon * range;
}

@fragment
fn fs_ambient_occlusion(
    in: FullscreenOut,
) -> @location(0) vec4f {
    let dimensions =
        vec2i(
            frame.viewport.xy
        );

    // Fullscreen fragments are guaranteed to lie inside the render target.
    let pixel =
        vec2i(
            in.position.xy
        );

    let center_depth =
        textureLoad(
            depth_texture,
            pixel,
            0,
        );

    if center_depth <= 0.0 {
        return vec4f(1.0);
    }

    let center =
        reconstruct_view_position(
            pixel,
            center_depth,
        );

    let normal =
        decode_shading_frame(
            textureLoad(
                normal_texture,
                pixel,
                0,
            ).xyz
        ).normal;

    var occlusion = 0.0;

    for (
        var index = 0u;
        index < 12u;
        index++
    ) {
        let sample_pixel =
            clamp(
                pixel +
                AO_OFFSETS[index],
                vec2i(0),
                dimensions - 1,
            );

        occlusion +=
            ambient_occlusion_sample(
                center,
                normal,
                sample_pixel,
            );
    }

    // occlusion is non-negative, so the unclamped value can never exceed 1.
    let visibility =
        max(
            1.0 -
                occlusion *
                AO_STRENGTH,
            AO_MIN_VISIBILITY,
        );

    return vec4f(
        visibility,
        contact_shadow(
            center,
            normal,
            dimensions,
        ),
        1.0,
        1.0,
    );
}
