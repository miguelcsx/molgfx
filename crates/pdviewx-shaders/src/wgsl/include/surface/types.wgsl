// Surface pipeline payloads, grid bindings and affine helpers.
//
// Every surface stage exchanges data through these records, so they are
// declared once here rather than duplicated per pipeline. The transform
// helpers drop the unused homogeneous W: surface work is affine, and the
// fourth row costs a multiply-add per fragment for a value never read.

const SURFACE_INFINITY: f32 = 1.0e20;
const SURFACE_RAY_EPSILON: f32 = 1.0e-7;
const SURFACE_NORMAL_EPSILON_SQ: f32 = 1.0e-10;
const SURFACE_MARCH_SAFETY: f32 = 0.9;

const SURFACE_CAP_TINT: vec3f = vec3f(0.68, 0.76, 0.82);
const SURFACE_CAP_ROUGHNESS: f32 = 0.82;
const SURFACE_CAP_MATERIAL: f32 = 0.05;

struct SurfaceHit {
    local_position: vec3f,
    local_normal: vec3f,
    compact_index: u32,
    valid: bool,
    cap: bool,
}

struct SurfaceVsOut {
    @builtin(position) position: vec4f,

    // Unnormalized model-local ray span. Interpolation happens in screen
    // space; normalization is required only once per fragment.
    @location(0) @interpolate(linear) ray_origin: vec3f,
    @location(1) @interpolate(linear) ray_vector: vec3f,
}

struct SurfaceRay {
    origin: vec3f,
    direction: vec3f,
    inverse_direction: vec3f,
}

struct SurfaceBounds {
    low: vec2f,
    high: vec2f,
}

struct GridCoordinate {
    lower: vec3u,
    fraction: vec3f,
}

struct GridCorners {
    c000: f32,
    c100: f32,
    c010: f32,
    c110: f32,
    c001: f32,
    c101: f32,
    c011: f32,
    c111: f32,
}

struct SurfaceFrame {
    world_position: vec3f,
    view_position: vec3f,
    view_normal: vec3f,
}

struct SurfaceMaterial {
    roughness: f32,
    material: f32,
}

@group(2) @binding(10) var surface_grid: texture_3d<f32>;
@group(2) @binding(11) var surface_provenance: texture_3d<u32>;

fn surface_miss() -> SurfaceHit {
    return SurfaceHit(
        vec3f(0.0),
        vec3f(0.0),
        EMPTY_COMPACT_INDEX,
        false,
        false,
    );
}

/// Returns four-vertex triangle-strip coordinates in [0, 1].
fn surface_quad_uv(vertex: u32) -> vec2f {
    return vec2f(
        f32(vertex & 1u),
        f32(vertex >> 1u),
    );
}

/// Transforms an affine point without computing an unused homogeneous W.
fn transform_point(
    matrix: mat4x4f,
    point: vec3f,
) -> vec3f {
    return matrix[0].xyz * point.x
        + matrix[1].xyz * point.y
        + matrix[2].xyz * point.z
        + matrix[3].xyz;
}

/// Transforms a direction without translation.
fn transform_direction(
    matrix: mat4x4f,
    direction: vec3f,
) -> vec3f {
    return matrix[0].xyz * direction.x
        + matrix[1].xyz * direction.y
        + matrix[2].xyz * direction.z;
}

/// Performs perspective division while preserving the sign of W.
fn homogeneous_point(value: vec4f) -> vec3f {
    return value.xyz * (
        sign(value.w) /
        max(abs(value.w), SURFACE_RAY_EPSILON)
    );
}
