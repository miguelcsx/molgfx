// Specialized depth-independent screen overlays composed after tonemapping.
//
// Pipelines:
//   glyph    -> vs_overlay_glyph    + fs_overlay_glyph
//   gradient -> vs_overlay_gradient + fs_overlay_gradient
//   scale    -> vs_overlay_scale    + fs_overlay_line
//   axis     -> vs_overlay_axis     + fs_overlay_line
//
// All overlays use a four-vertex triangle strip.

const GLYPH_COLUMNS: u32 = 5u;
const GLYPH_ROWS: u32 = 7u;
const GLYPH_ROW_MASK: u32 = 0x1Fu;

const GLYPH_SIZE: vec2f = vec2f(8.75, 13.65);
const GLYPH_STEP: vec2f = vec2f(1.75, 1.95);
const GLYPH_HALF_CELL: vec2f = vec2f(0.72, 0.82);

const GLYPH_INNER: f32 = -0.15;
const GLYPH_OUTER: f32 = 0.65;

const AXIS_EPSILON_SQ: f32 = 1.0e-10;

//!include "include/camera.wgsl"

struct OverlayGpu {
    anchor: vec4f,
    geometry: vec4f,
    color_a: vec4f,
    color_b: vec4f,
    metadata: vec4u,
}

@group(2) @binding(0) var<storage, read> overlays: array<OverlayGpu>;

struct GlyphVsOut {
    @builtin(position) position: vec4f,

    @location(0) local_point: vec2f,
    @location(1) @interpolate(flat, first) color: vec4f,
    @location(2) @interpolate(flat, first) bits: vec2u,
}

struct GradientVsOut {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
}

struct LineVsOut {
    @builtin(position) position: vec4f,

    @location(0) across: f32,
    @location(1) @interpolate(flat, first) color: vec4f,
}

/// Returns triangle-strip coordinates in [0, 1].
fn overlay_uv(vertex_index: u32) -> vec2f {
    return vec2f(
        f32(vertex_index & 1u),
        f32(vertex_index >> 1u),
    );
}

/// Vertices 0 and 2 provide flat data for the two strip triangles.
fn flat_source(vertex_index: u32) -> bool {
    return vertex_index == 0u || vertex_index == 2u;
}

/// Resolves an overlay anchor into pixel coordinates.
fn overlay_origin(anchor: vec4f) -> vec2f {
    return fma(
        anchor.xy,
        frame.viewport.xy,
        anchor.zw,
    );
}

/// Converts pixel coordinates directly to clip coordinates.
fn clip_from_pixel(pixel: vec2f) -> vec4f {
    return vec4f(
        fma(
            pixel,
            frame.viewport.zw * 2.0,
            vec2f(-1.0),
        ),
        0.0,
        1.0,
    );
}

/// Normalizes a 2D direction with the original +X fallback.
fn safe_direction(axis: vec2f) -> vec2f {
    let length_sq = dot(axis, axis);

    if length_sq <= AXIS_EPSILON_SQ {
        return vec2f(1.0, 0.0);
    }

    return axis * inverseSqrt(length_sq);
}

/// Computes the projected scale-bar length.
fn projected_scale_length(length: f32) -> f32 {
    let projected =
        length *
        frame.proj[0][0] *
        frame.viewport.x;

    if frame.projection_kind.x > 0.5 {
        return projected * 0.5;
    }

    return projected /
        max(
            2.0 * frame.illustration.w,
            1.0e-4,
        );
}

/// Returns the selected world basis axis already transformed into view XY.
fn view_axis(axis: u32) -> vec2f {
    if axis == 1u {
        return frame.view[1].xy;
    }

    if axis == 2u {
        return frame.view[2].xy;
    }

    return frame.view[0].xy;
}

fn box_sdf(
    point: vec2f,
    half_extent: vec2f,
) -> f32 {
    let delta =
        abs(point) - half_extent;

    return length(
        max(delta, vec2f(0.0))
    ) + min(
        max(delta.x, delta.y),
        0.0,
    );
}

/// Extracts one five-bit row from the packed 35-bit glyph bitmap.
fn glyph_row_bits(
    row: u32,
    low: u32,
    high: u32,
) -> u32 {
    if row < 6u {
        return (
            low >> (row * GLYPH_COLUMNS)
        ) & GLYPH_ROW_MASK;
    }

    return (
        (low >> 30u) |
        (high << 2u)
    ) & GLYPH_ROW_MASK;
}

/// Evaluates only active glyph cells instead of all 35 possible cells.
fn glyph_distance(
    point: vec2f,
    low: u32,
    high: u32,
) -> f32 {
    var result = 1.0e6;

    for (
        var row = 0u;
        row < GLYPH_ROWS;
        row++
    ) {
        var bits =
            glyph_row_bits(
                row,
                low,
                high,
            );

        let y =
            (f32(row) - 3.0) *
            GLYPH_STEP.y;

        while bits != 0u {
            let bit =
                firstTrailingBit(bits);

            let center =
                vec2f(
                    (2.0 - f32(bit)) *
                        GLYPH_STEP.x,
                    y,
                );

            result =
                min(
                    result,
                    box_sdf(
                        point - center,
                        GLYPH_HALF_CELL,
                    ),
                );

            // Coverage is already exactly 1 below this threshold.
            if result <= GLYPH_INNER {
                return result;
            }

            bits &= bits - 1u;
        }
    }

    return result;
}

// -----------------------------------------------------------------------------
// Glyph
// -----------------------------------------------------------------------------

@vertex
fn vs_overlay_glyph(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> GlyphVsOut {
    let anchor =
        overlays[instance_index].anchor;

    let geometry =
        overlays[instance_index].geometry.xy;

    let corner =
        overlay_uv(vertex_index);

    let origin =
        overlay_origin(anchor);

    let pixel =
        origin +
        geometry * (corner - vec2f(0.5));

    var out: GlyphVsOut;

    out.position =
        clip_from_pixel(pixel);

    out.local_point =
        (corner - vec2f(0.5)) *
        GLYPH_SIZE;

    out.color = vec4f(0.0);
    out.bits = vec2u(0u);

    if flat_source(vertex_index) {
        out.color =
            overlays[instance_index].color_a;

        out.bits =
            overlays[instance_index].metadata.yz;
    }

    return out;
}

@fragment
fn fs_overlay_glyph(
    in: GlyphVsOut,
) -> @location(0) vec4f {
    let distance =
        glyph_distance(
            in.local_point,
            in.bits.x,
            in.bits.y,
        );

    let coverage =
        1.0 -
        smoothstep(
            GLYPH_INNER,
            GLYPH_OUTER,
            distance,
        );

    if coverage <= 0.001 {
        discard;
    }

    return vec4f(
        in.color.rgb,
        in.color.a * coverage,
    );
}

// -----------------------------------------------------------------------------
// Gradient rectangle
// -----------------------------------------------------------------------------

@vertex
fn vs_overlay_gradient(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> GradientVsOut {
    let corner =
        overlay_uv(vertex_index);

    let pixel =
        overlay_origin(
            overlays[instance_index].anchor
        ) +
        overlays[instance_index].geometry.xy *
        corner;

    var color: vec4f;

    // Load only the endpoint color needed by this vertex.
    if (vertex_index & 1u) == 0u {
        color =
            overlays[instance_index].color_a;
    } else {
        color =
            overlays[instance_index].color_b;
    }

    var out: GradientVsOut;

    out.position =
        clip_from_pixel(pixel);

    // Raster interpolation now performs the old fragment mix().
    out.color =
        color;

    return out;
}

@fragment
fn fs_overlay_gradient(
    in: GradientVsOut,
) -> @location(0) vec4f {
    return in.color;
}

// -----------------------------------------------------------------------------
// Scale line
// -----------------------------------------------------------------------------

@vertex
fn vs_overlay_scale(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> LineVsOut {
    let geometry =
        overlays[instance_index].geometry;

    let corner =
        overlay_uv(vertex_index);

    let origin =
        overlay_origin(
            overlays[instance_index].anchor
        );

    let length =
        projected_scale_length(
            geometry.x
        );

    // Scale bars are always horizontal: no normalize(), axis or normal needed.
    let pixel =
        origin +
        vec2f(
            length * corner.x,
            geometry.y *
                (corner.y - 0.5),
        );

    var out: LineVsOut;

    out.position =
        clip_from_pixel(pixel);

    out.across =
        corner.y - 0.5;

    out.color =
        vec4f(0.0);

    if flat_source(vertex_index) {
        out.color =
            overlays[instance_index].color_a;
    }

    return out;
}

// -----------------------------------------------------------------------------
// View-axis line
// -----------------------------------------------------------------------------

@vertex
fn vs_overlay_axis(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> LineVsOut {
    let geometry =
        overlays[instance_index].geometry;

    let corner =
        overlay_uv(vertex_index);

    let origin =
        overlay_origin(
            overlays[instance_index].anchor
        );

    let direction =
        safe_direction(
            view_axis(
                overlays[instance_index].metadata.y
            )
        );

    let normal =
        vec2f(
            -direction.y,
            direction.x,
        );

    let pixel =
        origin +
        direction *
            geometry.x *
            corner.x +
        normal *
            geometry.y *
            (corner.y - 0.5);

    var out: LineVsOut;

    out.position =
        clip_from_pixel(pixel);

    out.across =
        corner.y - 0.5;

    out.color =
        vec4f(0.0);

    if flat_source(vertex_index) {
        out.color =
            overlays[instance_index].color_a;
    }

    return out;
}

// -----------------------------------------------------------------------------
// Shared line fragment
// -----------------------------------------------------------------------------

@fragment
fn fs_overlay_line(
    in: LineVsOut,
) -> @location(0) vec4f {
    let edge =
        1.0 -
        smoothstep(
            0.42,
            0.5,
            abs(in.across),
        );

    return vec4f(
        in.color.rgb,
        in.color.a * edge,
    );
}
