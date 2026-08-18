// Label payloads, projection and pixel-space quad helpers.
//
// All three pipelines are batched from one record array, so labels, guides
// and markers cost a single instanced draw between them. Positions are
// carried into pixel space once per vertex, which is what lets the fragment
// stage evaluate a distance field in stable pixel units at any zoom.

const DEPTH_BIAS: f32 = 1.0e-5;
const AXIS_EPSILON_SQ: f32 = 1.0e-6;

const GLYPH_COLUMNS: u32 = 5u;
const GLYPH_ROWS: u32 = 7u;
const GLYPH_ROW_MASK: u32 = 0x1Fu;

const GLYPH_STEP: vec2f = vec2f(1.75, 1.95);
const GLYPH_HALF_CELL: vec2f = vec2f(0.72, 0.82);
const GLYPH_FILL_INNER: f32 = -0.15;
const GLYPH_FILL_OUTER: f32 = 0.55;
const GLYPH_HALO_INNER: f32 = 0.75;
const GLYPH_HALO_OUTER: f32 = 1.65;

const GLYPH_HALO_COLOR: vec3f = vec3f(0.012, 0.018, 0.021);

struct LabelGpu {
    start_size: vec4f,
    end_offset: vec4f,
    color: vec4f,
    metadata: vec4u,
}

@group(2) @binding(0) var<storage, read> labels: array<LabelGpu>;

struct ProjectedPoint {
    pixel: vec2f,
    depth: f32,
}

struct GlyphVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(flat) center: vec2f,
    @location(1) @interpolate(flat) color: vec4f,

    // x = entity, y = structure, z/w = glyph bits.
    @location(2) @interpolate(flat) metadata: vec4u,
}

struct GuideVsOut {
    @builtin(position) position: vec4f,

    // xy = pixel start, zw = pixel axis.
    @location(0) @interpolate(flat) start_axis: vec4f,

    // x = inverse axis length squared
    // y = stroke width
    // z = start depth
    // w = depth delta
    @location(1) @interpolate(flat) metrics: vec4f,

    @location(2) @interpolate(flat) color: vec4f,
    @location(3) @interpolate(flat) ids: vec2u,
}

struct MarkerVsOut {
    @builtin(position) position: vec4f,

    // xy = center, z = radius.
    @location(0) @interpolate(flat) center_radius: vec3f,

    @location(1) @interpolate(flat) color: vec4f,

    // x = entity, y = structure, z = shape.
    @location(2) @interpolate(flat) metadata: vec3u,
}

struct LabelOutput {
    @location(0) accumulation: vec4f,
    @location(1) revealage: f32,
    @location(2) entity_id: u32,
    @location(3) structure_id: u32,
}

struct GuideOutput {
    @location(0) accumulation: vec4f,
    @location(1) revealage: f32,
    @location(2) entity_id: u32,
    @location(3) structure_id: u32,
    @builtin(frag_depth) depth: f32,
}

/// Projects one world-space point directly to pixel space.
fn project_point(point: vec3f) -> ProjectedPoint {
    let clip =
        frame.view_proj *
        vec4f(point, 1.0);

    let inv_w =
        1.0 / clip.w;

    let ndc =
        clip.xy * inv_w;

    return ProjectedPoint(
        (
            ndc * vec2f(0.5, -0.5) +
            vec2f(0.5)
        ) * frame.viewport.xy,

        clip.z * inv_w,
    );
}

/// Returns triangle-strip coordinates in [0,1].
fn quad_uv(vertex_index: u32) -> vec2f {
    return vec2f(
        f32(vertex_index & 1u),
        f32(vertex_index >> 1u),
    );
}

/// Expands a pixel-space rectangle.
fn quad_pixel(
    low: vec2f,
    high: vec2f,
    vertex_index: u32,
) -> vec2f {
    return mix(
        low,
        high,
        quad_uv(vertex_index),
    );
}

/// Converts pixel coordinates to clip coordinates.
fn clip_from_pixel(
    pixel: vec2f,
    depth: f32,
) -> vec4f {
    let ndc =
        pixel *
        frame.viewport.zw *
        vec2f(2.0, -2.0) +
        vec2f(-1.0, 1.0);

    return vec4f(
        ndc,
        depth,
        1.0,
    );
}
