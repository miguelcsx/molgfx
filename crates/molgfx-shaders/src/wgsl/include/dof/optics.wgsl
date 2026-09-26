// Thin-lens circle of confusion and polygonal aperture sampling.
//
// The aperture taps are a fixed golden-angle set baked as a constant, so the
// gather costs no per-pixel sample generation and stays deterministic frame
// to frame. Signed radius keeps near and far blur distinguishable, which is
// what lets the resolve reject occluders rather than smear across them.

const TILE_SIZE: i32 = 16;
const GATHER_TAPS: u32 = 32u;

const PI: f32 = 3.141592653589793;
const TWO_PI: f32 = 6.283185307179586;

const SHARP_RADIUS_PIXELS: f32 = 0.5;
const MIN_VIEW_DEPTH: f32 = 1.0e-4;

// Retained for the aperture sequence definition.
const GOLDEN_ANGLE: f32 = 2.399963229728653;
const APERTURE_EXTENT: f32 = 0.72;

// Precomputed Vogel samples.
//
// xy = aperture sample
// z  = atan2(y, x)
//
// This removes sqrt/sin/cos from aperture_sample() and lets the resolve
// avoid atan2() as well.
const APERTURE_TAPS: array<vec4f, 32> = array<vec4f, 32>(
    vec4f( 0.090000004,  0.000000000,  0.000000000, 0.0),
    vec4f(-0.114944436,  0.105298519,  2.399963140, 0.0),
    vec4f( 0.017594088, -0.200475559, -1.483258840, 0.0),
    vec4f( 0.144880012,  0.188970327,  0.916704357, 0.0),
    vec4f(-0.265872627, -0.047029126, -2.966517690, 0.0),
    vec4f( 0.251857787, -0.160211295, -0.566554487, 0.0),
    vec4f(-0.084241495,  0.313374162,  1.833408710, 0.0),
    vec4f(-0.160657674, -0.309336573, -2.049813270, 0.0),
    vec4f( 0.348562896,  0.127294600,  0.350149930, 0.0),
    vec4f(-0.362621605,  0.149684921,  2.750113250, 0.0),
    vec4f( 0.174807578, -0.373553634, -1.133108970, 0.0),
    vec4f( 0.129178345,  0.411840945,  1.266854290, 0.0),
    vec4f(-0.389345050, -0.225633413, -2.616367820, 0.0),
    vec4f( 0.456746072, -0.100414336, -0.216404542, 0.0),
    vec4f(-0.278744996,  0.396486104,  2.183558700, 0.0),
    vec4f(-0.064396553, -0.496943742, -1.699663400, 0.0),
    vec4f( 0.395331651,  0.333185941,  0.700299859, 0.0),
    vec4f(-0.531992495,  0.021999560,  3.100263120, 0.0),
    vec4f( 0.388047695, -0.386159271, -0.782959044, 0.0),
    vec4f(-0.025961895,  0.561449885,  1.617004160, 0.0),
    vec4f(-0.369228631, -0.442459285, -2.266217950, 0.0),
    vec4f( 0.584898889,  0.078697324,  0.133745372, 0.0),
    vec4f(-0.495583653,  0.344814211,  2.533708570, 0.0),
    vec4f( 0.135421962, -0.601964176, -1.349513530, 0.0),
    vec4f( 0.313223958,  0.546617568,  1.050449730, 0.0),
    vec4f(-0.612322867, -0.195347697, -2.832772250, 0.0),
    vec4f( 0.594793737, -0.274809778, -0.432809085, 0.0),
    vec4f(-0.257679492,  0.615711987,  1.967154150, 0.0),
    vec4f(-0.229973286, -0.639384329, -1.916067960, 0.0),
    vec4f( 0.611934245,  0.321615487,  0.483895272, 0.0),
    vec4f(-0.679704964,  0.179168046,  2.883858440, 0.0),
    vec4f( 0.386348993, -0.600861430, -0.999363542, 0.0),
);

struct DofViewDepthParams {
    pixel_x_zw: vec2f,
    pixel_y_zw: vec2f,
    depth_zw: vec2f,
    bias_zw: vec2f,
}

struct DofCocParams {
    focus_distance: f32,
    sensor_scale: f32,
    max_radius: f32,
}

struct DofPolygonParams {
    sector: f32,
    half_sector: f32,
    boundary: f32,
}

@group(1) @binding(0) var classify_depth: texture_2d<f32>;

@group(2) @binding(0) var resolved_hdr: texture_2d<f32>;
@group(2) @binding(1) var resolved_depth: texture_2d<f32>;
@group(2) @binding(2) var tile_coc: texture_2d<f32>;

fn dof_view_depth_params(
    dimensions: vec2i,
) -> DofViewDepthParams {
    let pixel_scale =
        vec2f(2.0, -2.0) /
        vec2f(dimensions);

    let ndc_bias =
        vec2f(-1.0, 1.0) +
        pixel_scale * 0.5;

    return DofViewDepthParams(
        frame.inv_proj[0].zw * pixel_scale.x,
        frame.inv_proj[1].zw * pixel_scale.y,
        frame.inv_proj[2].zw,
        frame.inv_proj[0].zw * ndc_bias.x
            + frame.inv_proj[1].zw * ndc_bias.y
            + frame.inv_proj[3].zw,
    );
}

fn dof_depth_from_zw(
    view_zw: vec2f,
) -> f32 {
    let inv_w =
        sign(view_zw.y) /
        max(
            abs(view_zw.y),
            MIN_VIEW_DEPTH,
        );

    return max(
        -view_zw.x * inv_w,
        MIN_VIEW_DEPTH,
    );
}

fn dof_view_depth(
    pixel: vec2i,
    depth: f32,
    params: DofViewDepthParams,
) -> f32 {
    let view_zw =
        params.bias_zw
        + params.pixel_x_zw * f32(pixel.x)
        + params.pixel_y_zw * f32(pixel.y)
        + params.depth_zw * depth;

    return dof_depth_from_zw(
        view_zw,
    );
}

fn view_depth(
    pixel: vec2i,
    depth: f32,
    dimensions: vec2i,
) -> f32 {
    return dof_view_depth(
        pixel,
        depth,
        dof_view_depth_params(
            dimensions,
        ),
    );
}

fn dof_coc_params() -> DofCocParams {
    return DofCocParams(
        max(
            frame.optics.x,
            MIN_VIEW_DEPTH,
        ),
        frame.optics.y *
            frame.viewport.x *
            0.5,
        frame.optics.z,
    );
}

fn dof_signed_coc_radius(
    depth: f32,
    params: DofCocParams,
) -> f32 {
    // view_depth() already guarantees depth >= MIN_VIEW_DEPTH.
    let normalized_defocus =
        1.0 -
        params.focus_distance / depth;

    return clamp(
        normalized_defocus *
            params.sensor_scale,
        -params.max_radius,
        params.max_radius,
    );
}

fn signed_coc_radius(
    depth: f32,
) -> f32 {
    return dof_signed_coc_radius(
        max(
            depth,
            MIN_VIEW_DEPTH,
        ),
        dof_coc_params(),
    );
}

fn aperture_sample(
    index: u32,
) -> vec2f {
    return APERTURE_TAPS[index].xy;
}

fn dof_polygon_params(
    blades: f32,
) -> DofPolygonParams {
    let blade_count =
        max(
            blades,
            3.0,
        );

    let sector =
        TWO_PI /
        blade_count;

    let half_sector =
        sector * 0.5;

    return DofPolygonParams(
        sector,
        half_sector,
        cos(half_sector),
    );
}

fn dof_polygon_scale(
    angle: f32,
    params: DofPolygonParams,
) -> f32 {
    let local_angle =
        (
            (
                angle +
                params.half_sector
            ) %
            params.sector
        ) -
        params.half_sector;

    return params.boundary /
        max(
            cos(local_angle),
            0.25,
        );
}

fn polygonal_sample(
    sample_point: vec2f,
    blades: f32,
) -> vec2f {
    let params =
        dof_polygon_params(
            blades,
        );

    return sample_point *
        dof_polygon_scale(
            atan2(
                sample_point.y,
                sample_point.x,
            ),
            params,
        );
}

fn dof_aperture_sample(
    index: u32,
    polygon: DofPolygonParams,
) -> vec2f {
    let tap =
        APERTURE_TAPS[index];

    return tap.xy *
        dof_polygon_scale(
            tap.z,
            polygon,
        );
}
