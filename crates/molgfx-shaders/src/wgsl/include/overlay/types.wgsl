// Screen-overlay records, kind constants and shared drawing helpers.
//
// Overlays are batched into one instance array and drawn once per kind; the
// kind in metadata.x selects which specialized pipeline keeps an instance and
// which collapse it to a zero-area primitive. The projection helpers place
// glyphs, scale bars and axes in stable screen pixels so they hold their size
// under any camera.
//
//!include "include/camera.wgsl"

const GLYPH_COLUMNS: u32 = 5u;
const GLYPH_ROWS: u32 = 7u;
const GLYPH_ROW_MASK: u32 = 0x1Fu;

const GLYPH_SIZE: vec2f = vec2f(8.75, 13.65);
const GLYPH_STEP: vec2f = vec2f(1.75, 1.95);
const GLYPH_HALF_CELL: vec2f = vec2f(0.72, 0.82);

const GLYPH_INNER: f32 = -0.15;
const GLYPH_OUTER: f32 = 0.65;

const AXIS_EPSILON_SQ: f32 = 1.0e-10;

struct OverlayGpu {
    anchor: vec4f,
    geometry: vec4f,
    color_a: vec4f,
    color_b: vec4f,
    metadata: vec4u,
}

@group(2) @binding(0) var<storage, read> overlays: array<OverlayGpu>;

// Overlay kind, carried in metadata.x.
const OVERLAY_KIND_GLYPH: u32 = 0u;
const OVERLAY_KIND_GRADIENT: u32 = 1u;
const OVERLAY_KIND_SCALE: u32 = 2u;
const OVERLAY_KIND_AXIS: u32 = 3u;

/// A clip position outside the view volume. Every vertex of a mismatched
/// instance collapses here, giving a zero-area primitive with no fragments, so
/// the kind-specialized pipelines share one instance array and each skips the
/// kinds it does not draw.
const OVERLAY_CULLED: vec4f = vec4f(2.0, 2.0, 2.0, 1.0);

struct GlyphVsOut {
    @builtin(position) position: vec4f,

    @location(0) local_point: vec2f,
    @location(1) @interpolate(flat, either) color: vec4f,
    @location(2) @interpolate(flat, either) bits: vec2u,
}

struct GradientVsOut {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
}

struct LineVsOut {
    @builtin(position) position: vec4f,

    @location(0) across: f32,
    @location(1) @interpolate(flat, either) color: vec4f,
}

/// Returns triangle-list coordinates in [0, 1].
fn overlay_uv(vertex_index: u32) -> vec2f {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return corners[min(vertex_index, 5u)];
}

/// Vertices 0 and 3 provide flat data for the two independent triangles.
fn flat_source(vertex_index: u32) -> bool {
    return vertex_index == 0u || vertex_index == 3u;
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
