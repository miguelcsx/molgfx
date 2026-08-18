// Weighted-blended transparency composite in linear HDR.
//
// Fully opaque-background pixels skip the accumulation texture entirely.
// Fully transparent-covered pixels skip the opaque HDR fetch.

//!include "include/fullscreen.wgsl"

@group(1) @binding(0) var opaque_hdr: texture_2d<f32>;
@group(1) @binding(1) var oit_accumulation: texture_2d<f32>;
@group(1) @binding(2) var oit_revealage: texture_2d<f32>;

@fragment
fn fs_oit_composite(
    in: FullscreenOut,
) -> @location(0) vec4f {
    let pixel =
        vec2i(in.position.xy);

    let revealage =
        clamp(
            textureLoad(
                oit_revealage,
                pixel,
                0,
            ).r,
            0.0,
            1.0,
        );

    // Typical background pixel: one revealage + one opaque fetch.
    if revealage >= 1.0 {
        return vec4f(
            textureLoad(
                opaque_hdr,
                pixel,
                0,
            ).rgb,
            1.0,
        );
    }

    let accumulation =
        textureLoad(
            oit_accumulation,
            pixel,
            0,
        );

    let transparent =
        accumulation.rgb /
        max(
            accumulation.a,
            1.0e-5,
        );

    if revealage <= 0.0 {
        return vec4f(
            transparent,
            1.0,
        );
    }

    let opaque =
        textureLoad(
            opaque_hdr,
            pixel,
            0,
        ).rgb;

    return vec4f(
        mix(
            transparent,
            opaque,
            revealage,
        ),
        1.0,
    );
}
