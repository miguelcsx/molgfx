// Specialized depth-independent screen overlays composed after tonemapping.
//
// Pipelines:
//   glyph    -> vs_overlay_glyph    + fs_overlay_glyph
//   gradient -> vs_overlay_gradient + fs_overlay_gradient
//   scale    -> vs_overlay_scale    + fs_overlay_line
//   axis     -> vs_overlay_axis     + fs_overlay_line
//
// All overlays use six-vertex triangle-list quads.
//
// This file holds the four kind pipelines; the records, kind constants and
// shared screen-space helpers live beside it under include/overlay/.

//!include "include/camera.wgsl"
//!include "include/overlay/types.wgsl"

@vertex
fn vs_overlay_glyph(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> GlyphVsOut {
    let overlay = overlays[instance_index];
    if overlay.metadata.x != OVERLAY_KIND_GLYPH {
        var culled: GlyphVsOut;
        culled.position = OVERLAY_CULLED;
        return culled;
    }

    let anchor = overlay.anchor;

    let geometry = overlay.geometry.xy;

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
        out.color = overlay.color_a;

        out.bits = overlay.metadata.yz;
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
    let overlay = overlays[instance_index];
    if overlay.metadata.x != OVERLAY_KIND_GRADIENT {
        var culled: GradientVsOut;
        culled.position = OVERLAY_CULLED;
        return culled;
    }

    let corner =
        overlay_uv(vertex_index);

    let pixel =
        overlay_origin(
            overlay.anchor
        ) +
        overlay.geometry.xy *
        corner;

    var color: vec4f;

    // Load only the endpoint color needed by this vertex.
    if (vertex_index & 1u) == 0u {
        color =
            overlay.color_a;
    } else {
        color =
            overlay.color_b;
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
    let overlay = overlays[instance_index];
    if overlay.metadata.x != OVERLAY_KIND_SCALE {
        var culled: LineVsOut;
        culled.position = OVERLAY_CULLED;
        return culled;
    }

    let geometry = overlay.geometry;

    let corner =
        overlay_uv(vertex_index);

    let origin =
        overlay_origin(
            overlay.anchor
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
            overlay.color_a;
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
    let overlay = overlays[instance_index];
    if overlay.metadata.x != OVERLAY_KIND_AXIS {
        var culled: LineVsOut;
        culled.position = OVERLAY_CULLED;
        return culled;
    }

    let geometry = overlay.geometry;

    let corner =
        overlay_uv(vertex_index);

    let origin =
        overlay_origin(
            overlay.anchor
        );

    let direction =
        safe_direction(
            view_axis(
                overlay.metadata.y
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
            overlay.color_a;
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
