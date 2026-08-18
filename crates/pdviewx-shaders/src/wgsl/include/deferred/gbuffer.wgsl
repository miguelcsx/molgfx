// Gbuffer bindings, tuning constants and view reconstruction.
//
// View position is rebuilt from depth rather than stored, so the gbuffer
// carries one depth attachment instead of a full position target — a
// bandwidth saving paid on every pixel of every frame.

@group(1) @binding(0) var albedo_texture: texture_2d<f32>;
@group(1) @binding(1) var normal_texture: texture_2d<f32>;
@group(1) @binding(2) var depth_texture: texture_depth_2d;
@group(1) @binding(3) var ao_texture: texture_2d<f32>;
@group(1) @binding(4) var shadow_texture: texture_depth_2d;

const ILLUSTRATION_OFFSETS: array<vec2i, 4> = array<vec2i, 4>(
    vec2i(1, 0),
    vec2i(-1, 0),
    vec2i(0, 1),
    vec2i(0, -1),
);

const EDGE_DEPTH_LOW: f32 = 0.002;
const EDGE_DEPTH_HIGH: f32 = 0.018;
const EDGE_NORMAL_LOW: f32 = 0.08;
const EDGE_NORMAL_HIGH: f32 = 0.42;

const CAVITY_LOW: f32 = 0.0005;
const CAVITY_HIGH: f32 = 0.012;

const MIN_VIEW_DEPTH_SCALE: f32 = 0.25;
const VIEW_W_EPSILON: f32 = 1.0e-7;

const CROSS_SAMPLE_WEIGHT: f32 = 0.25;

const MAX_SILHOUETTE_DARKENING: f32 = 0.82;
const MAX_CAVITY_DARKENING: f32 = 0.38;

const DEPTH_CUE_NEAR_FOCUS: f32 = 1.15;
const DEPTH_CUE_FAR_FOCUS: f32 = 2.75;
const MIN_FOCUS_DISTANCE: f32 = 1.0e-3;

const SHADOW_BIAS: f32 = 0.0015;
const SHADOW_NORMAL_OFFSET: f32 = 0.04;
const SHADOW_TAP_COUNT: u32 = 4u;
const SHADOW_INV_TAP_COUNT: f32 = 0.25;

const SHADOW_OFFSETS: array<vec2i, 4> = array<vec2i, 4>(
    vec2i(-1, -1),
    vec2i(1, -1),
    vec2i(-1, 1),
    vec2i(1, 1),
);

struct IllustrationDepthParams {
    pixel_x_zw: vec2f,
    pixel_y_zw: vec2f,
    depth_zw: vec2f,
    bias_zw: vec2f,
}

fn illustration_depth_params(
    dimensions: vec2i,
) -> IllustrationDepthParams {
    let scale =
        vec2f(2.0, -2.0) /
        vec2f(dimensions);

    let bias =
        vec2f(-1.0, 1.0) +
        scale * 0.5;

    return IllustrationDepthParams(
        frame.inv_proj[0].zw * scale.x,
        frame.inv_proj[1].zw * scale.y,
        frame.inv_proj[2].zw,
        frame.inv_proj[0].zw * bias.x
            + frame.inv_proj[1].zw * bias.y
            + frame.inv_proj[3].zw,
    );
}

fn illustration_pixel_zw(
    pixel: vec2i,
    params: IllustrationDepthParams,
) -> vec2f {
    return params.bias_zw
        + params.pixel_x_zw * f32(pixel.x)
        + params.pixel_y_zw * f32(pixel.y);
}

fn illustration_view_z(
    pixel_zw: vec2f,
    depth: f32,
    params: IllustrationDepthParams,
) -> f32 {
    let zw =
        pixel_zw +
        params.depth_zw * depth;

    return zw.x *
        sign(zw.y) /
        max(
            abs(zw.y),
            VIEW_W_EPSILON,
        );
}

fn view_position(
    pixel: vec2i,
    depth: f32,
    dimensions: vec2i,
) -> vec3f {
    let scale =
        vec2f(2.0, -2.0) /
        vec2f(dimensions);

    let ndc_xy =
        vec2f(pixel) * scale
        + vec2f(-1.0, 1.0)
        + scale * 0.5;

    let view =
        frame.inv_proj[0] * ndc_xy.x
        + frame.inv_proj[1] * ndc_xy.y
        + frame.inv_proj[2] * depth
        + frame.inv_proj[3];

    return view.xyz *
        (
            sign(view.w) /
            max(
                abs(view.w),
                VIEW_W_EPSILON,
            )
        );
}

fn background(
    uv: vec2f,
) -> vec3f {
    let top =
        frame.atmosphere[0].rgb;

    let bottom =
        frame.atmosphere[1].rgb;

    var offset =
        uv -
        frame.atmosphere[3].zw;

    offset.x *=
        frame.viewport.x /
        max(
            frame.viewport.y,
            1.0,
        );

    let pool =
        1.0 -
        smoothstep(
            0.0,
            0.95,
            length(offset),
        );

    return mix(
        top,
        bottom,
        smoothstep(
            0.0,
            1.0,
            uv.y,
        ),
    ) +
        pool *
        pool *
        frame.atmosphere[2].rgb *
        frame.atmosphere[0].w;
}
