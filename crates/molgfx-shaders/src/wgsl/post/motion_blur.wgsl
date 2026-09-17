// Deterministic nine-tap motion blur.
//
// The center sample is reused for both RGB accumulation and output alpha.
// Four symmetric sample pairs replace the generic nine-iteration loop.
//
// Contract:
//   - source_texture and motion_vectors match frame.viewport.
//   - source_sampler uses clamp-to-edge addressing.

//!include "include/fullscreen.wgsl"
//!include "include/camera.wgsl"

@group(1) @binding(0) var source_texture: texture_2d<f32>;
@group(1) @binding(1) var motion_vectors: texture_2d<f32>;
@group(1) @binding(2) var source_sampler: sampler;

const MOTION_BLUR_INV_SAMPLES: f32 = 1.0 / 9.0;

fn sample_source(uv: vec2f) -> vec4f {
    return textureSampleLevel(
        source_texture,
        source_sampler,
        uv,
        0.0,
    );
}

@fragment
fn fs_motion_blur(
    in: FullscreenOut,
) -> @location(0) vec4f {
    let pixel =
        vec2i(in.position.xy);

    let motion =
        textureLoad(
            motion_vectors,
            pixel,
            0,
        ).xy;

    let maximum =
        frame.motion_blur.y *
        frame.viewport.zw;

    let delta =
        clamp(
            motion *
                frame.motion_blur.x,
            -maximum,
            maximum,
        );

    let delta_pixels =
        delta *
        frame.viewport.xy;

    let center =
        sample_source(in.uv);

    if dot(
        delta_pixels,
        delta_pixels
    ) < 0.25 {
        return center;
    }

    var color =
        center.rgb;

    for (
        var tap = 1u;
        tap <= 4u;
        tap++
    ) {
        let amount =
            f32(tap) * 0.125;

        let offset =
            delta * amount;

        color +=
            sample_source(
                in.uv + offset
            ).rgb +
            sample_source(
                in.uv - offset
            ).rgb;
    }

    return vec4f(
        color *
            MOTION_BLUR_INV_SAMPLES,
        center.a,
    );
}
