// Specialized analytic stroke-SDF labels, measurement guides and markers.
//
// Labels are batched by metadata.z:
//   0 = glyph
//   1 = guide
//   2+ = marker
//
// Each instance expands to a six-vertex triangle-list quad.
//
// This file holds the three pipelines; the payloads they share and the
// distance fields they sample live beside it under include/label/.

//!include "include/camera.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/label/types.wgsl"
//!include "include/label/sdf.wgsl"

// -----------------------------------------------------------------------------
// Glyph pipeline
// -----------------------------------------------------------------------------

@vertex
fn vs_label_glyph(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> GlyphVsOut {
    let label =
        labels[instance_index];

    if label_kind(label) != LABEL_KIND_GLYPH {
        var culled: GlyphVsOut;
        culled.position = LABEL_CULLED;
        return culled;
    }

    let anchor =
        project_point(
            label.start_size.xyz,
        );

    let center =
        anchor.pixel +
        label.end_offset.xy;

    let half_extent =
        vec2f(
            label.start_size.w,
            label.end_offset.w,
        ) * 0.5 +
        vec2f(2.5);

    let pixel =
        quad_pixel(
            center - half_extent,
            center + half_extent,
            vertex_index,
        );

    var out: GlyphVsOut;

    out.position =
        clip_from_pixel(
            pixel,
            clamp(
                anchor.depth + DEPTH_BIAS,
                0.0,
                1.0,
            ),
        );

    out.center =
        center;

    out.color =
        label.color;

    out.metadata =
        vec4u(
            label.metadata.x,
            label.metadata.y,
            label.metadata.w,
            bitcast<u32>(
                label.end_offset.z
            ),
        );

    return out;
}

@fragment
fn fs_label_glyph(
    in: GlyphVsOut,
) -> LabelOutput {
    let sdf =
        glyph_distance(
            in.position.xy - in.center,
            in.metadata.z,
            in.metadata.w,
        );

    let fill =
        1.0 -
        smoothstep(
            GLYPH_FILL_INNER,
            GLYPH_FILL_OUTER,
            sdf,
        );

    let halo =
        1.0 -
        smoothstep(
            GLYPH_HALO_INNER,
            GLYPH_HALO_OUTER,
            sdf,
        );

    let coverage =
        max(
            fill,
            halo * 0.82,
        );

    if coverage <= 0.001 {
        discard;
    }

    let color =
        mix(
            GLYPH_HALO_COLOR,
            in.color.rgb,
            fill,
        );

    let transparency =
        weighted_transparency(
            color,
            in.color.a * coverage,
            in.position.z,
        );

    var out: LabelOutput;

    out.accumulation =
        transparency.accumulation;

    out.revealage =
        transparency.revealage;

    out.entity_id =
        in.metadata.x;

    out.structure_id =
        in.metadata.y;

    return out;
}

// -----------------------------------------------------------------------------
// Measurement guide pipeline
// -----------------------------------------------------------------------------

@vertex
fn vs_label_guide(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> GuideVsOut {
    let label =
        labels[instance_index];

    if label_kind(label) != LABEL_KIND_GUIDE {
        var culled: GuideVsOut;
        culled.position = LABEL_CULLED;
        return culled;
    }

    let start =
        project_point(
            label.start_size.xyz,
        );

    let end =
        project_point(
            label.end_offset.xyz,
        );

    let axis =
        end.pixel -
        start.pixel;

    let width =
        label.start_size.w;

    let extent =
        vec2f(width + 2.0);

    let pixel =
        quad_pixel(
            min(start.pixel, end.pixel) -
                extent,
            max(start.pixel, end.pixel) +
                extent,
            vertex_index,
        );

    var out: GuideVsOut;

    // The true line depth is authored by the fragment stage.
    out.position =
        clip_from_pixel(
            pixel,
            0.0,
        );

    out.start_axis =
        vec4f(
            start.pixel,
            axis,
        );

    out.metrics =
        vec4f(
            1.0 /
                max(
                    dot(axis, axis),
                    AXIS_EPSILON_SQ,
                ),
            width,
            start.depth,
            end.depth - start.depth,
        );

    out.color =
        label.color;

    out.ids =
        label.metadata.xy;

    return out;
}

@fragment
fn fs_label_guide(
    in: GuideVsOut,
) -> GuideOutput {
    let relative =
        in.position.xy -
        in.start_axis.xy;

    let axis =
        in.start_axis.zw;

    let along =
        clamp(
            dot(relative, axis) *
                in.metrics.x,
            0.0,
            1.0,
        );

    let delta =
        relative -
        axis * along;

    let distance_sq =
        dot(delta, delta);

    let width =
        in.metrics.y;

    let outer =
        width + 1.0;

    if distance_sq >= outer * outer {
        discard;
    }

    var coverage = 1.0;

    // sqrt() is paid only in the one-pixel anti-alias transition.
    if distance_sq > width * width {
        coverage =
            1.0 -
            smoothstep(
                width,
                outer,
                sqrt(distance_sq),
            );

        if coverage <= 0.001 {
            discard;
        }
    }

    let depth =
        clamp(
            fma(
                in.metrics.w,
                along,
                in.metrics.z,
            ) + DEPTH_BIAS,
            0.0,
            1.0,
        );

    let transparency =
        weighted_transparency(
            in.color.rgb,
            in.color.a * coverage,
            depth,
        );

    var out: GuideOutput;

    out.accumulation =
        transparency.accumulation;

    out.revealage =
        transparency.revealage;

    out.entity_id =
        in.ids.x;

    out.structure_id =
        in.ids.y;

    out.depth =
        depth;

    return out;
}

// -----------------------------------------------------------------------------
// Marker pipeline
// -----------------------------------------------------------------------------

@vertex
fn vs_label_marker(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> MarkerVsOut {
    let label =
        labels[instance_index];

    if label_kind(label) != LABEL_KIND_MARKER {
        var culled: MarkerVsOut;
        culled.position = LABEL_CULLED;
        return culled;
    }

    let anchor =
        project_point(
            label.start_size.xyz,
        );

    let radius =
        label.start_size.w;

    let extent =
        vec2f(radius + 2.0);

    let pixel =
        quad_pixel(
            anchor.pixel - extent,
            anchor.pixel + extent,
            vertex_index,
        );

    var out: MarkerVsOut;

    out.position =
        clip_from_pixel(
            pixel,
            clamp(
                anchor.depth + DEPTH_BIAS,
                0.0,
                1.0,
            ),
        );

    out.center_radius =
        vec3f(
            anchor.pixel,
            radius,
        );

    out.color =
        label.color;

    out.metadata =
        vec3u(
            label.metadata.x,
            label.metadata.y,
            label.metadata.w,
        );

    return out;
}

@fragment
fn fs_label_marker(
    in: MarkerVsOut,
) -> LabelOutput {
    let coverage =
        marker_coverage(
            in.position.xy -
                in.center_radius.xy,
            in.center_radius.z,
            in.metadata.z,
        );

    if coverage <= 0.001 {
        discard;
    }

    let transparency =
        weighted_transparency(
            in.color.rgb,
            in.color.a * coverage,
            in.position.z,
        );

    var out: LabelOutput;

    out.accumulation =
        transparency.accumulation;

    out.revealage =
        transparency.revealage;

    out.entity_id =
        in.metadata.x;

    out.structure_id =
        in.metadata.y;

    return out;
}
