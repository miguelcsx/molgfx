// Pixel-stable analytic molecular interaction glyphs.
//
// Each interaction expands to a four-vertex triangle strip.
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

struct InteractionVsOut {
    @builtin(position) position: vec4f,

    // xy = pixel start, zw = normalized screen axis.
    @location(0) @interpolate(flat) pixel_start_axis: vec4f,

    // x = length, y = inverse length, z = radius, w = phase / period.
    @location(1) @interpolate(flat) metrics: vec4f,

    // x = inverse period, y = duty, z = arrow size, w = length / period.
    @location(2) @interpolate(flat) style: vec4f,

    // xy = start clip ZW, zw = end-start clip ZW.
    @location(3) @interpolate(flat) depth_zw: vec4f,

    // Alpha already includes the glyph opacity multiplier.
    @location(4) @interpolate(flat) color: vec4f,

    // x = entity, y = structure, z = pattern, w = directional.
    @location(5) @interpolate(flat) metadata: vec4u,
}

struct InteractionHit {
    along: f32,
    depth: f32,
    valid: bool,
}

/// Returns triangle-strip quad coordinates in [0, 1].
fn interaction_quad_uv(vertex_index: u32) -> vec2f {
    return vec2f(
        f32(vertex_index & 1u),
        f32(vertex_index >> 1u),
    );
}

/// Resolves the main solid, dashed, dotted or spring stroke.
fn interaction_main_visible(
    in: InteractionVsOut,
    raw_along: f32,
    along: f32,
    perpendicular: f32,
) -> bool {
    let radius = in.metrics.z;
    let pattern = in.metadata.z;

    // Spring is the only path that pays for sin().
    if pattern == 3u {
        let phase =
            raw_along * in.style.x +
            in.metrics.w;

        let spring =
            sin(phase * TAU) *
            radius * 2.2;

        return abs(perpendicular - spring) <= radius;
    }

    if abs(perpendicular) > radius {
        return false;
    }

    if pattern == 0u {
        return true;
    }

    let cell = fract(
        along * in.style.w +
        in.metrics.w
    );

    if pattern == 1u {
        return cell <= in.style.y;
    }

    return abs(cell - 0.5) <= radius * in.style.x;
}

/// Tests the optional direction marker.
fn interaction_arrow_visible(
    in: InteractionVsOut,
    raw_along: f32,
    perpendicular: f32,
) -> bool {
    if in.metadata.w == 0u {
        return false;
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
        return false;
    }

    return abs(
        abs(perpendicular) -
        from_tip * 0.62
    ) <= max(
        radius * 0.72,
        0.75,
    );
}

/// Resolves coverage and true projected depth.
fn resolve_interaction(in: InteractionVsOut) -> InteractionHit {
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

    if !interaction_main_visible(
        in,
        raw_along,
        along,
        perpendicular,
    ) && !interaction_arrow_visible(
        in,
        raw_along,
        perpendicular,
    ) {
        return InteractionHit(
            0.0,
            0.0,
            false,
        );
    }

    let zw =
        in.depth_zw.xy +
        in.depth_zw.zw * along;

    return InteractionHit(
        along,
        zw.x / zw.y,
        true,
    );
}

@vertex
fn vs_interaction(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> InteractionVsOut {
    let start_width =
        interactions[instance_index].start_width;

    let end_period =
        interactions[instance_index].end_period;

    let arrow_size =
        interactions[instance_index].style.w;

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

    let pixel_start =
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

    let length =
        length_squared *
        inverse_length;

    let unit_axis =
        pixel_axis *
        inverse_length;

    let interaction_style =
        interactions[instance_index].style;

    let inverse_period =
        1.0 /
        max(end_period.w, 1.0);

    let phase_pixels =
        interaction_style.z +
        interactions[instance_index].animation.x *
        frame.temporal.w;

    let color =
        interactions[instance_index].color;

    let metadata =
        interactions[instance_index].metadata;

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
            inverse_length,
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

    out.depth_zw =
        vec4f(
            clip_start.zw,
            clip_end.zw - clip_start.zw,
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
            metadata.w & 3u,
        );

    return out;
}

struct InteractionOutput {
    @location(0) accumulation: vec4f,
    @location(1) revealage: f32,
    @location(2) entity_id: u32,
    @location(3) structure_id: u32,
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
            in.color.a,
            hit.depth,
        );

    var out: InteractionOutput;

    out.accumulation =
        transparency.accumulation;

    out.revealage =
        transparency.revealage;

    out.entity_id =
        in.metadata.x;

    out.structure_id =
        in.metadata.y;

    out.depth =
        hit.depth;

    return out;
}
