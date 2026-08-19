// Bond payloads, projection and strip-quad helpers.
//
// Both pipelines exchange data through these records and project through
// these helpers, so the shared cost is paid once. Projection drops the unused
// homogeneous rows: a bond endpoint needs affine XYZ and clip XYW, never the
// full four-by-four product, which is a saved multiply-add per vertex.

const BOND_LINE_AXIS_EPSILON_SQ: f32 = 1.0e-8;
const BOND_AXIS_EPSILON_SQ: f32 = 1.0e-12;
const BOND_MIN_VIEW_DEPTH: f32 = 1.0e-4;

const BOND_LINE_NORMAL: vec3f = vec3f(0.0, 0.0, 1.0);
const BOND_CAP_TINT: vec3f = vec3f(0.68, 0.76, 0.82);
const BOND_CAP_ROUGHNESS: f32 = 0.82;
const BOND_CAP_MATERIAL: f32 = 0.05;

struct BondLineVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(flat, first) endpoint_a: vec3f,
    @location(1) @interpolate(flat, first) endpoint_axis: vec3f,

    // xy = pixel A, zw = pixel-space axis.
    @location(2) @interpolate(flat, first) pixel_a_axis: vec4f,

    @location(3) @interpolate(flat, first) color_a: vec4f,
    @location(4) @interpolate(flat, first) color_delta: vec4f,

    // xy = motion A, zw = motion delta.
    @location(5) @interpolate(flat, first) motion_a_delta: vec4f,

    // x = roughness
    // y = inverse pixel-axis length squared
    // z = half-width squared
    // w = material payload
    @location(6) @interpolate(flat, first) aux: vec4f,

    @location(7) @interpolate(flat, first) entity_id: u32,
}

struct BondCapsuleVsOut {
    @builtin(position) position: vec4f,

    // ray.z is invariant -1.0.
    @location(0) ray_xy: vec2f,

    // xyz = endpoint A, w = radius.
    @location(1) @interpolate(flat, first) endpoint_a_radius: vec4f,

    // xyz = endpoint B, w = inverse axis length squared.
    @location(2) @interpolate(flat, first) endpoint_b_inv_axis_sq: vec4f,

    @location(3) @interpolate(flat, first) color_a: vec4f,
    @location(4) @interpolate(flat, first) color_delta: vec4f,
    @location(5) @interpolate(flat, first) motion_a_delta: vec4f,

    // x = regular roughness
    // y = clipping-cap roughness
    // z = inverse radius
    // w = regular material payload
    @location(6) @interpolate(flat, first) aux: vec4f,

    @location(7) @interpolate(flat, first) entity_id: u32,
}

struct BondLineHit {
    position: vec3f,
    along: f32,
    valid: bool,
}

struct BondCapsuleHit {
    position: vec3f,
    normal: vec3f,
    along: f32,
    cap: bool,
    valid: bool,
}

struct BondMaterialData {
    base: vec3f,
    roughness: f32,
    material: f32,
}

struct BondFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) structure_id: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

/// Transforms world position to view space without computing W.
fn bond_view_position(world: vec3f) -> vec3f {
    return frame.view[0].xyz * world.x
        + frame.view[1].xyz * world.y
        + frame.view[2].xyz * world.z
        + frame.view[3].xyz;
}

/// Transforms view position back to world space.
fn bond_world_position(view: vec3f) -> vec3f {
    return frame.inv_view[0].xyz * view.x
        + frame.inv_view[1].xyz * view.y
        + frame.inv_view[2].xyz * view.z
        + frame.inv_view[3].xyz;
}

/// Projects a view position directly to NDC XY.
fn bond_project_ndc(view: vec3f) -> vec2f {
    let xyw =
        frame.proj[0].xyw * view.x
        + frame.proj[1].xyw * view.y
        + frame.proj[2].xyw * view.z
        + frame.proj[3].xyw;

    return xyw.xy * (1.0 / xyw.z);
}

/// Computes depth from a true view-space hit.
fn bond_view_depth(view: vec3f) -> f32 {
    let zw =
        frame.proj[0].zw * view.x
        + frame.proj[1].zw * view.y
        + frame.proj[2].zw * view.z
        + frame.proj[3].zw;

    return zw.x / zw.y;
}

/// Returns triangle-list quad coordinates in [0, 1].
fn bond_quad_uv(vertex_index: u32) -> vec2f {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return corners[min(vertex_index, 5u)];
}

/// Returns true for the provoking vertex of each independent triangle.
fn bond_flat_source(vertex_index: u32) -> bool {
    return vertex_index == 0u || vertex_index == 3u;
}

/// Expands two projected endpoints into a screen rectangle.
fn bond_quad_ndc(
    a: vec2f,
    b: vec2f,
    extent: vec2f,
    vertex_index: u32,
) -> vec2f {
    return mix(
        min(a, b) - extent,
        max(a, b) + extent,
        bond_quad_uv(vertex_index),
    );
}

/// Packs pixel A and the screen-space endpoint axis.
fn bond_pixel_a_axis(
    ndc_a: vec2f,
    ndc_b: vec2f,
) -> vec4f {
    let scale =
        frame.viewport.xy *
        vec2f(0.5, -0.5);

    return vec4f(
        ndc_a * scale + frame.viewport.xy * 0.5,
        (ndc_b - ndc_a) * scale,
    );
}

/// Computes the capsule's conservative NDC radius.
fn bond_capsule_extent(
    a: vec3f,
    b: vec3f,
    radius: f32,
) -> vec2f {
    let depth = max(
        min(-a.z, -b.z),
        BOND_MIN_VIEW_DEPTH,
    );

    return vec2f(
        frame.proj[0][0],
        frame.proj[1][1],
    ) * (radius / depth);
}

fn bond_color(
    start: vec4f,
    delta: vec4f,
    along: f32,
) -> vec4f {
    return fma(
        delta,
        vec4f(along),
        start,
    );
}

fn bond_motion(
    packed: vec4f,
    along: f32,
) -> vec2f {
    return fma(
        packed.zw,
        vec2f(along),
        packed.xy,
    );
}
