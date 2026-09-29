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
@group(1) @binding(1) var depth_texture: texture_2d<f32>;
@group(1) @binding(2) var revealage_texture: texture_2d<f32>;
@group(1) @binding(3) var bloom_texture: texture_2d<f32>;

// Required pipeline constants matching the old encoded atmosphere tag.
override PRESENTATION_GAMUT_TAG: f32;
override PRESENTATION_TRANSFER_TAG: f32;

// Create a no-bloom variant when bloom is disabled.
override TONEMAP_BLOOM_ENABLED: u32 = 1u;

// Fused edge anti-aliasing. It runs on the encoded image, where edges are
// perceptually uniform, and costs five presentations of a pixel instead of
// one, so only the variant built for it pays. Both thresholds are contrast in
// encoded luma: below the floor an edge is invisible noise, and above the
// ceiling it is fully smoothed.
override FXAA_ENABLED: u32 = 0u;
const FXAA_EDGE_FLOOR: f32 = 0.0312;
const FXAA_EDGE_CEILING: f32 = 0.25;
const FXAA_RELATIVE_FLOOR: f32 = 0.125;

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

/// The encoded display colour of one pixel, before coverage is attached.
fn present_color(
    pixel: vec2i,
    uv: vec2f,
) -> vec3f {
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
                uv
            );
    }

    var display =
        tone_map(hdr);

    display =
        apply_display_adjustments(
            display,
            uv,
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

    return clamp(
        display,
        vec3f(0.0),
        vec3f(1.0),
    );
}

fn present_luma(color: vec3f) -> f32 {
    return dot(color, LUMINANCE_WEIGHTS);
}

/// Smooths stair-stepped edges by blending each pixel with its two neighbours
/// along the edge, weighted by how strong the edge is.
///
/// The gradient of encoded luma across the four axis neighbours gives the edge
/// normal; the blend runs along the tangent, so the edge keeps its position and
/// only its steps are averaged. A pixel with no local contrast returns
/// unchanged after the five presentations.
fn present_color_smoothed(
    pixel: vec2i,
    uv: vec2f,
    dimensions: vec2i,
) -> vec3f {
    let texel = 1.0 / vec2f(dimensions);
    let last = dimensions - vec2i(1);
    let east_pixel = min(pixel + vec2i(1, 0), last);
    let west_pixel = max(pixel - vec2i(1, 0), vec2i(0));
    let south_pixel = min(pixel + vec2i(0, 1), last);
    let north_pixel = max(pixel - vec2i(0, 1), vec2i(0));

    let centre = present_color(pixel, uv);
    let east = present_color(east_pixel, uv + vec2f(texel.x, 0.0));
    let west = present_color(west_pixel, uv - vec2f(texel.x, 0.0));
    let south = present_color(south_pixel, uv + vec2f(0.0, texel.y));
    let north = present_color(north_pixel, uv - vec2f(0.0, texel.y));

    let luma_centre = present_luma(centre);
    let luma_east = present_luma(east);
    let luma_west = present_luma(west);
    let luma_south = present_luma(south);
    let luma_north = present_luma(north);

    let highest = max(
        luma_centre,
        max(max(luma_east, luma_west), max(luma_south, luma_north)),
    );
    let lowest = min(
        luma_centre,
        min(min(luma_east, luma_west), min(luma_south, luma_north)),
    );
    let contrast = highest - lowest;
    if contrast < max(FXAA_EDGE_FLOOR, highest * FXAA_RELATIVE_FLOOR) {
        return centre;
    }

    let gradient = vec2f(luma_east - luma_west, luma_south - luma_north);
    let along_vertical_edge = abs(gradient.x) >= abs(gradient.y);
    let neighbours = select(
        (east + west) * 0.5,
        (north + south) * 0.5,
        along_vertical_edge,
    );
    let strength = smoothstep(FXAA_EDGE_FLOOR, FXAA_EDGE_CEILING, contrast);
    return mix(centre, mix(centre, neighbours, 0.5), strength);
}

@fragment
fn fs_tonemap(
    in: FullscreenOut,
) -> @location(0) vec4f {
    // Contract: presentation inputs match frame.viewport.
    let pixel =
        vec2i(in.position.xy);

    var display: vec3f;
    if FXAA_ENABLED != 0u {
        display = present_color_smoothed(
            pixel,
            in.uv,
            vec2i(textureDimensions(hdr_texture)),
        );
    } else {
        display = present_color(pixel, in.uv);
    }

    return vec4f(
        display,
        presentation_coverage(
            pixel,
            in.uv.y,
        ),
    );
}
