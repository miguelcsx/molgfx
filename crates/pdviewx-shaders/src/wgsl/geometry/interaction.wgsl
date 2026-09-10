// Pixel-stable analytic molecular interaction glyphs.
//
// Each interaction expands to a six-vertex triangle-list quad.
// Per-interaction geometry is flat and precomputed in the vertex stage;
// fragments only resolve the 2D pattern, optional marker and analytic depth.

//!include "include/camera.wgsl"
//!include "include/material_lighting.wgsl"

const AXIS_EPSILON_SQ: f32 = 1.0e-6;
const TAU: f32 = 6.2831853;

struct InteractionGpu {
    start_width: vec4f,
    end_period: vec4f,
    color: vec4f,
    metadata: vec4u,
    style: vec4f,
    animation: vec4f,
}

@group(2) @binding(0) var<storage, read> interactions: array<InteractionGpu>;
@group(2) @binding(1) var<storage, read> visible_interactions: array<u32>;

struct InteractionVsOut {
    @builtin(position) position: vec4f,

    // xy = pixel start, zw = normalized screen axis.
    @location(0) @interpolate(flat) pixel_start_axis: vec4f,

    // x = length, y = inverse length, z = radius, w = phase / period.
    @location(1) @interpolate(flat) metrics: vec4f,

    // x = inverse period, y = duty, z = arrow size, w = length / period.
    @location(2) @interpolate(flat) style: vec4f,

    // x = start NDC depth, y = end-start NDC depth.
    @location(3) @interpolate(flat) depth_zw: vec4f,

    // Alpha already includes the glyph opacity multiplier.
    @location(4) @interpolate(flat) color: vec4f,

    // x = entity, y = structure, z = pattern, w = directional.
    @location(5) @interpolate(flat) metadata: vec4u,
}

struct InteractionHit {
    along: f32,
    depth: f32,
    coverage: f32,
    valid: bool,
}

const COVERAGE_FEATHER_PIXELS: f32 = 0.75;

/// Returns triangle-list quad coordinates in [0, 1].
fn interaction_quad_uv(vertex_index: u32) -> vec2f {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return corners[min(vertex_index, 5u)];
}

/// Resolves the main solid, dashed, dotted or spring stroke.
fn stroke_coverage(distance_pixels: f32, radius_pixels: f32) -> f32 {
    return 1.0 - smoothstep(
        max(radius_pixels - COVERAGE_FEATHER_PIXELS, 0.0),
        radius_pixels + COVERAGE_FEATHER_PIXELS,
        distance_pixels,
    );
}

fn interaction_main_coverage(
    in: InteractionVsOut,
    raw_along: f32,
    along: f32,
    perpendicular: f32,
) -> f32 {
    let radius = in.metrics.z;
    let pattern = in.metadata.z;
    let endpoint_excess = max(
        max(-raw_along, raw_along - in.metrics.x),
        0.0,
    );
    let endpoint_coverage = stroke_coverage(endpoint_excess, radius);

    // Spring is the only path that pays for sin().
    if pattern == 3u {
        let phase =
            raw_along * in.style.x +
            in.metrics.w;

        let spring =
            sin(phase * TAU) *
            radius * 2.2;

        return stroke_coverage(abs(perpendicular - spring), radius) * endpoint_coverage;
    }

    let transverse = stroke_coverage(abs(perpendicular), radius);

    if pattern == 0u {
        return transverse * endpoint_coverage;
    }

    let cell = fract(
        along * in.style.w +
        in.metrics.w
    );

    if pattern == 1u {
        let edge = in.style.x * COVERAGE_FEATHER_PIXELS;
        return transverse * endpoint_coverage *
            (1.0 - smoothstep(in.style.y - edge, in.style.y + edge, cell));
    }

    let dot_radius = radius * in.style.x;
    let edge = in.style.x * COVERAGE_FEATHER_PIXELS;
    let longitudinal = 1.0 - smoothstep(
        max(dot_radius - edge, 0.0),
        dot_radius + edge,
        abs(cell - 0.5),
    );
    return transverse * longitudinal * endpoint_coverage;
}

/// Tests the optional direction marker.
fn interaction_arrow_coverage(
    in: InteractionVsOut,
    raw_along: f32,
    perpendicular: f32,
) -> f32 {
    if (in.metadata.w & 3u) == 0u {
        return 0.0;
    }

    let radius = in.metrics.z;
    let arrow_size = in.style.z;

    let marker_tip =
        in.metrics.x -
        max(
            arrow_size * 0.72,
            radius * 2.5,
        );

    let from_tip =
        marker_tip - raw_along;

    if from_tip < 0.0 || from_tip > arrow_size {
        return 0.0;
    }

    return stroke_coverage(abs(
        abs(perpendicular) -
        from_tip * 0.62
    ), max(
        radius * 0.72,
        0.75,
    ));
}

/// Resolves coverage and true projected depth.
fn resolve_interaction(in: InteractionVsOut) -> InteractionHit {
    if in.metrics.x <= 1.0 {
        return InteractionHit(0.0, 0.0, 0.0, false);
    }
    let relative =
        in.position.xy -
        in.pixel_start_axis.xy;

    let axis =
        in.pixel_start_axis.zw;

    let raw_along =
        dot(relative, axis);

    let along =
        clamp(
            raw_along * in.metrics.y,
            0.0,
            1.0,
        );

    let perpendicular =
        axis.x * relative.y -
        axis.y * relative.x;

    let coverage = max(interaction_main_coverage(
        in,
        raw_along,
        along,
        perpendicular,
    ), interaction_arrow_coverage(
        in,
        raw_along,
        perpendicular,
    ));
    if coverage <= 1.0e-4 {
        return InteractionHit(
            0.0,
            0.0,
            0.0,
            false,
        );
    }

    let interpolated_depth = in.depth_zw.x + in.depth_zw.y *
        (in.depth_zw.z + in.depth_zw.w * along);
    let depth = select(
        interpolated_depth,
        0.0,
        (in.metadata.w & 4u) != 0u,
    );
    return InteractionHit(
        along,
        depth,
        coverage,
        true,
    );
}

@vertex
fn vs_interaction(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> InteractionVsOut {
    let relation_index = visible_interactions[instance_index];
    let start_width =
        interactions[relation_index].start_width;

    let end_period =
        interactions[relation_index].end_period;

    let arrow_size =
        interactions[relation_index].style.w;

    // view_proj replaces separate world->view and view->clip transforms.
    let clip_start =
        frame.view_proj *
        vec4f(start_width.xyz, 1.0);

    let clip_end =
        frame.view_proj *
        vec4f(end_period.xyz, 1.0);

    let ndc_start =
        clip_start.xy *
        (1.0 / clip_start.w);

    let ndc_end =
        clip_end.xy *
        (1.0 / clip_end.w);

    let radius =
        start_width.w * 0.5;

    let extent =
        (radius + arrow_size * 0.75 + 1.0) *
        2.0 *
        frame.viewport.zw;

    let ndc =
        mix(
            min(ndc_start, ndc_end) - extent,
            max(ndc_start, ndc_end) + extent,
            interaction_quad_uv(vertex_index),
        );

    let pixel_scale =
        frame.viewport.xy *
        vec2f(0.5, -0.5);

    let anchor_pixel_start =
        ndc_start * pixel_scale +
        frame.viewport.xy * 0.5;

    let pixel_axis =
        (ndc_end - ndc_start) *
        pixel_scale;

    let length_squared =
        max(
            dot(pixel_axis, pixel_axis),
            AXIS_EPSILON_SQ,
        );

    // One inverse square root supplies both length and normalized axis.
    let inverse_length =
        inverseSqrt(length_squared);

    let anchor_length =
        length_squared *
        inverse_length;

    let unit_axis =
        pixel_axis *
        inverse_length;

    let relation_animation = interactions[relation_index].animation;
    let start_inset = min(relation_animation.y, anchor_length * 0.5);
    let end_inset = min(relation_animation.z, max(anchor_length - start_inset, 0.0));
    let length = max(anchor_length - start_inset - end_inset, 0.0);
    let pixel_start = anchor_pixel_start + unit_axis * start_inset;

    let interaction_style =
        interactions[relation_index].style;

    let inverse_period =
        1.0 /
        max(end_period.w, 1.0);

    let phase_pixels =
        interaction_style.z +
        relation_animation.x *
        frame.temporal.w;

    let color =
        interactions[relation_index].color;

    let metadata =
        interactions[relation_index].metadata;

    var out: InteractionVsOut;

    out.position =
        vec4f(
            ndc,
            0.0,
            1.0,
        );

    out.pixel_start_axis =
        vec4f(
            pixel_start,
            unit_axis,
        );

    out.metrics =
        vec4f(
            length,
        1.0 / max(length, 1.0e-3),
            radius,
            phase_pixels * inverse_period,
        );

    out.style =
        vec4f(
            inverse_period,
            interaction_style.y,
            arrow_size,
            length * inverse_period,
        );

    let start_depth = clip_start.z / clip_start.w;
    let end_depth = clip_end.z / clip_end.w;
    let inverse_anchor_length = 1.0 / max(anchor_length, 1.0e-3);
    out.depth_zw = vec4f(
        start_depth,
        end_depth - start_depth,
        start_inset * inverse_anchor_length,
        length * inverse_anchor_length,
    );

    out.color =
        vec4f(
            color.rgb,
            color.a * interaction_style.x,
        );

    out.metadata =
        vec4u(
            metadata.x,
            metadata.y,
            metadata.z,
            (metadata.w & 3u) |
                select(0u, 4u, relation_animation.w > 0.5),
        );

    return out;
}

struct InteractionOutput {
    @location(0) accumulation: vec4f,
    @location(1) revealage: f32,
    @location(2) entity_id: u32,
    @location(3) resident_page: u32,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_interaction(
    in: InteractionVsOut,
) -> InteractionOutput {
    let hit =
        resolve_interaction(in);

    if !hit.valid {
        discard;
    }

    let transparency =
        weighted_transparency(
            in.color.rgb,
            in.color.a * hit.coverage,
            hit.depth,
        );

    var out: InteractionOutput;

    out.accumulation =
        transparency.accumulation;

    out.revealage =
        transparency.revealage;

    out.entity_id =
        in.metadata.x & 0x0fffffffu;

    out.resident_page =
        in.metadata.y;

    out.depth =
        hit.depth;

    return out;
}
