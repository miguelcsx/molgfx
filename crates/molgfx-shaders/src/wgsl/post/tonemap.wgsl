// Deterministic HDR presentation with compile-time display specialization.
//
// Gamut and transfer encoding are pipeline constants rather than per-pixel
// decoded tags. Create one pipeline variant for each active output format.

//!include "include/fullscreen.wgsl"
//!include "include/camera.wgsl"
//!include "include/tonemap/bloom.wgsl"
//!include "include/tonemap/display.wgsl"
//!include "include/tonemap/present.wgsl"

@group(1) @binding(0) var hdr_texture: texture_2d<f32>;
@group(1) @binding(1) var depth_texture: texture_depth_2d;
@group(1) @binding(2) var revealage_texture: texture_2d<f32>;
@group(1) @binding(3) var bloom_texture: texture_2d<f32>;

// Required pipeline constants matching the old encoded atmosphere tag.
override PRESENTATION_GAMUT_TAG: f32;
override PRESENTATION_TRANSFER_TAG: f32;

// Create a no-bloom variant when bloom is disabled.
override TONEMAP_BLOOM_ENABLED: u32 = 1u;

const SRGB_THRESHOLD: f32 = 0.0031308;
const SRGB_POWER: f32 = 1.0 / 2.4;

const HLG_THRESHOLD: f32 = 1.0 / 12.0;
const HLG_A: f32 = 0.17883277;
const HLG_B: f32 = 0.28466892;
const HLG_C: f32 = 0.55991073;
const HLG_LOG_EPSILON: f32 = 1.0e-6;

const PQ_M1: f32 = 2610.0 / 16384.0;
const PQ_M2: f32 = 2523.0 / 32.0;
const PQ_C1: f32 = 3424.0 / 4096.0;
const PQ_C2: f32 = 2413.0 / 128.0;
const PQ_C3: f32 = 2392.0 / 128.0;
const PQ_INV_MAX_NITS: f32 = 1.0 / 10000.0;

const LUMINANCE_WEIGHTS: vec3f =
    vec3f(0.2126, 0.7152, 0.0722);

const CONTRAST_PIVOT: vec3f =
    vec3f(0.18);

@fragment
fn fs_tonemap(
    in: FullscreenOut,
) -> @location(0) vec4f {
    // Contract: presentation inputs match frame.viewport.
    let pixel =
        vec2i(in.position.xy);

    var hdr =
        textureLoad(
            hdr_texture,
            pixel,
            0,
        ).rgb *
        frame.atmosphere[1].w;

    if TONEMAP_BLOOM_ENABLED != 0u {
        hdr +=
            bloom_contribution(
                in.uv
            );
    }

    var display =
        tone_map(hdr);

    display =
        apply_display_adjustments(
            display,
            in.uv,
        );

    display =
        display_gamut_tagged(
            display,
            PRESENTATION_GAMUT_TAG,
        );

    display =
        encode_transfer_tagged(
            display,
            PRESENTATION_TRANSFER_TAG,
        );

    return vec4f(
        clamp(
            display,
            vec3f(0.0),
            vec3f(1.0),
        ),
        presentation_coverage(
            pixel,
            in.uv.y,
        ),
    );
}
