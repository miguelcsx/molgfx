// Deterministic temporal anti-aliasing over the linear HDR frame.
//
// The spatial fallback and 3x3 history clamp share the same cardinal samples.
// Accepted history therefore requires exactly nine current-HDR texel loads:
// center + four cardinals + four diagonals.
//
// Rejected history needs only the five-tap spatial cross.

//!include "include/camera.wgsl"
//!include "include/fullscreen.wgsl"

@group(1) @binding(0) var current_hdr: texture_2d<f32>;
@group(1) @binding(1) var current_depth: texture_depth_2d;
@group(1) @binding(2) var history_hdr_depth: texture_2d<f32>;
@group(1) @binding(3) var history_sampler: sampler;
@group(1) @binding(4) var motion_vectors: texture_2d<f32>;

//!include "include/post/temporal_sampling.wgsl"

@fragment
@diagnostic(off, derivative_uniformity)
fn fs_temporal_resolve(
    in: FullscreenOut,
) -> @location(0) vec4f {
    // Contract: all full-resolution temporal inputs match frame.viewport.
    let dimensions =
        vec2i(frame.viewport.xy);

    let pixel =
        vec2i(in.position.xy);

    let depth =
        textureLoad(
            current_depth,
            pixel,
            0,
        );

    let current =
        textureLoad(
            current_hdr,
            pixel,
            0,
        ).rgb;

    let cross =
        temporal_cross(
            pixel,
            dimensions,
            current,
        );

    if frame.temporal.x < 0.5 {
        return vec4f(
            cross.fallback,
            depth,
        );
    }

    let uv =
        (vec2f(pixel) + vec2f(0.5)) *
        frame.viewport.zw;

    let current_clip =
        vec4f(
            uv *
                vec2f(2.0, -2.0) +
                vec2f(-1.0, 1.0),
            depth,
            1.0,
        );

    let previous_clip =
        frame.reprojection *
        current_clip;

    if previous_clip.w <= 0.0 {
        return vec4f(
            cross.fallback,
            depth,
        );
    }

    let previous_ndc =
        previous_clip.xyz *
        (1.0 / previous_clip.w);

    var previous_uv =
        vec2f(
            previous_ndc.x * 0.5 + 0.5,
            0.5 - previous_ndc.y * 0.5,
        );

    // Background has no object-motion vector worth reading.
    if depth > 0.0 {
        previous_uv =
            uv +
            textureLoad(
                motion_vectors,
                pixel,
                0,
            ).xy;
    }

    if any(
        previous_uv <= vec2f(0.0)
    ) || any(
        previous_uv >= vec2f(1.0)
    ) {
        return vec4f(
            cross.fallback,
            depth,
        );
    }

    // Strict UV bounds guarantee this conversion remains in-range.
    let history_pixel =
        vec2i(
            previous_uv *
            vec2f(dimensions)
        );

    let history_nearest =
        textureLoad(
            history_hdr_depth,
            history_pixel,
            0,
        );

    let expected_depth =
        previous_ndc.z;

    let depth_tolerance =
        max(
            0.0015,
            abs(dpdx(expected_depth)) +
            abs(dpdy(expected_depth)),
        );

    // Under reversed-Z a larger stored depth is a nearer surface. A filtered
    // background lookup turns subpixel motion into a continuous trail instead
    // of preserving a chain of discrete former silhouettes. Its carried depth
    // decays with colour, so the existing HDR ping-pong retains a bounded tail
    // without an age texture.
    // Foreground keeps the nearest-depth rejection below, so filtering cannot
    // pull an occluded surface through a current one.
    let disocclusion = frame.npr.y;
    if disocclusion > 0.0 && depth <= depth_tolerance {
        let history_soft =
            textureSampleLevel(
                history_hdr_depth,
                history_sampler,
                previous_uv,
                0.0,
            );
        if history_soft.a > expected_depth + depth_tolerance {
            let ghost =
                min(
                    disocclusion * DISOCCLUSION_TRAIL_STRENGTH,
                    0.85,
                );
            return vec4f(
                mix(cross.fallback, history_soft.rgb, ghost),
                history_soft.a * ghost,
            );
        }
    }

    if abs(
        history_nearest.a -
        expected_depth
    ) > depth_tolerance {
        // Remaining depth disagreement is a true rejection. Any current
        // foreground reaches this path and immediately overwrites old history.
        return vec4f(
            cross.fallback,
            depth,
        );
    }

    // Only accepted history pays for the remaining four neighborhood loads.
    let bounds =
        temporal_neighborhood_bounds(
            pixel,
            dimensions,
            cross,
        );

    let history =
        textureSampleLevel(
            history_hdr_depth,
            history_sampler,
            previous_uv,
            0.0,
        ).rgb;

    let progressive_weight =
        frame.temporal.w /
        (
            frame.temporal.w +
            1.0
        );

    let current_luma =
        luma(current);

    if frame.temporal.y > 0.5 {
        let illumination_ratio =
            clamp(
                luma(history) /
                max(
                    current_luma,
                    1.0e-4,
                ),
                0.5,
                1.5,
            );

        let quality_history =
            clamp(
                current *
                    illumination_ratio,
                bounds.minimum,
                bounds.maximum,
            );

        return vec4f(
            mix(
                cross.fallback,
                quality_history,
                progressive_weight,
            ),
            depth,
        );
    }

    let clamped_history =
        clamp(
            history,
            bounds.minimum,
            bounds.maximum,
        );

    let disagreement =
        abs(
            luma(clamped_history) -
            current_luma
        );

    // The luminance term is already zero outside this range.
    if disagreement >= 0.35 {
        return vec4f(
            cross.fallback,
            depth,
        );
    }

    let luminance_weight =
        1.0 -
        smoothstep(
            0.08,
            0.35,
            disagreement,
        );

    let chroma_delta =
        clamped_history -
        current;

    let chroma_sq =
        dot(
            chroma_delta,
            chroma_delta,
        );

    if chroma_sq >=
        CHROMA_REJECT_SQ {
        return vec4f(
            cross.fallback,
            depth,
        );
    }

    var chroma_weight = 1.0;

    // Avoid sqrt/smoothstep when chroma is already in the full-weight region.
    if chroma_sq >
        CHROMA_FULL_WEIGHT_SQ {
        chroma_weight =
            1.0 -
            smoothstep(
                0.10,
                0.28,
                sqrt(chroma_sq),
            );
    }

    let history_weight =
        progressive_weight *
        luminance_weight *
        chroma_weight;

    let resolved =
        mix(
            cross.fallback,
            clamped_history,
            history_weight,
        );

    // Ghosting trails: where accepted history overlaps this fragment (a
    // flexible loop whipping across itself), retain the un-clamped history in
    // proportion to on-screen speed so mobile regions smear while rigid ones
    // stay crisp. Disabled at zero persistence, so ordinary TAA is unchanged.
    // ponytail: no disocclusion trail behind fast rigid bodies; that needs a
    // separate depth-independent accumulation buffer.
    let persistence = frame.npr.y;
    if persistence > 0.0 && depth > 0.0 {
        let speed =
            length(
                textureLoad(motion_vectors, pixel, 0).xy *
                vec2f(dimensions),
            );
        let trail =
            min(
                persistence * smoothstep(0.5, 6.0, speed),
                0.92,
            );
        return vec4f(
            mix(resolved, history, trail),
            depth,
        );
    }

    return vec4f(
        resolved,
        depth,
    );
}
