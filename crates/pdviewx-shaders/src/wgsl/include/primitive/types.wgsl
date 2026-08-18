// Caller-authored primitive records, payloads and shared ray setup.
//
// One record describes every shape, so a scene of mixed primitives stays a
// single instanced draw. The camera ray is built once per fragment and the
// oriented ray rotates it into a primitive's local frame, which lets each
// shape solve in its own axis-aligned space instead of a general one.

const PRIMITIVE_INFINITY: f32 = 1.0e30;
const PRIMITIVE_DIRECTION_EPSILON: f32 = 1.0e-7;
const PRIMITIVE_QUADRATIC_EPSILON: f32 = 1.0e-7;

const PRIMITIVE_ROUGHNESS: f32 = 0.42;
const PRIMITIVE_MATERIAL: f32 = 0.35;

const PRIMITIVE_PRIMITIVE_ELLIPSOID: u32 = 0u;
const PRIMITIVE_PRIMITIVE_POLYGON: u32 = 1u;
const PRIMITIVE_PRIMITIVE_PARTICLE: u32 = 3u;

const PENTAGON: array<vec2f, 5> = array<vec2f, 5>(
    vec2f(0.0, 1.0),
    vec2f(-0.95105652, 0.30901699),
    vec2f(-0.58778525, -0.80901699),
    vec2f(0.58778525, -0.80901699),
    vec2f(0.95105652, 0.30901699),
);

const HEXAGON: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(0.0, 1.0),
    vec2f(-0.86602540, 0.5),
    vec2f(-0.86602540, -0.5),
    vec2f(0.0, -1.0),
    vec2f(0.86602540, -0.5),
    vec2f(0.86602540, 0.5),
);

struct PrimitiveGpu {
    center_radius: vec4f,
    orientation: vec4f,
    size_opacity: vec4f,
    inverse_primary: vec4f,
    inverse_cross: vec4f,
    color: vec4f,
    metadata: vec4u,
}

@group(2) @binding(0)
var<storage, read> primitive: array<PrimitiveGpu>;

@group(2) @binding(1)
var<storage, read> primitive_previous: array<vec4f>;

struct PrimitiveVsOut {
    @builtin(position) position: vec4f,

    @location(0) view_position: vec3f,

    @location(1) @interpolate(flat, first) world_center: vec3f,
    @location(2) @interpolate(flat, first) radius: f32,
    @location(3) @interpolate(flat, first) orientation: vec4f,
    @location(4) @interpolate(flat, first) size: vec3f,
    @location(5) @interpolate(flat, first) inverse_primary: vec4f,
    @location(6) @interpolate(flat, first) inverse_cross: vec4f,
    @location(7) @interpolate(flat, first) color: vec4f,
    @location(8) @interpolate(flat, first) metadata: vec4u,
    @location(9) @interpolate(flat, first) previous_world_center: vec3f,
}

struct PrimitiveHit {
    t: f32,
    normal_world: vec3f,
    weight: f32,
    valid: bool,
}

struct PrimitiveRay {
    origin: vec3f,
    direction: vec3f,
}

struct PrimitiveFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) structure_id: u32,
    @location(4) motion: vec2f,
    @builtin(frag_depth) depth: f32,
}

fn primitive_miss() -> PrimitiveHit {
    return PrimitiveHit(
        -1.0,
        vec3f(0.0),
        0.0,
        false,
    );
}

/// Returns four-vertex triangle-strip corners in [-1, 1].
fn primitive_corner(vertex: u32) -> vec2f {
    return vec2f(
        f32(vertex & 1u),
        f32(vertex >> 1u),
    ) * 2.0 - vec2f(1.0);
}

/// Transforms an affine point without computing the unused homogeneous W.
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

/// Computes true device depth from a view-space hit.
fn primitive_view_depth(view: vec3f) -> f32 {
    let zw =
        frame.proj[0].zw * view.x
        + frame.proj[1].zw * view.y
        + frame.proj[2].zw * view.z
        + frame.proj[3].zw;

    return zw.x / zw.y;
}

/// Builds the camera ray exactly once per fragment.
fn primitive_ray(
    view_position: vec3f,
) -> PrimitiveRay {
    if frame.projection_kind.x > 0.5 {
        return PrimitiveRay(
            frame.inv_view[0].xyz * view_position.x
                + frame.inv_view[1].xyz * view_position.y
                + frame.inv_view[3].xyz,

            normalize(
                -frame.inv_view[2].xyz
            ),
        );
    }

    return PrimitiveRay(
        frame.inv_view[3].xyz,

        normalize(
            transform_direction(
                frame.inv_view,
                view_position,
            )
        ),
    );
}

struct OrientedRay {
    origin: vec3f,
    direction: vec3f,
    orientation: vec4f,
    half_size: vec3f,
}

/// Transforms a world ray once into primitive-local space.
///
/// orientation is expected to be normalized by the caller.
fn oriented_ray(
    in: PrimitiveVsOut,
    ray: PrimitiveRay,
) -> OrientedRay {
    let inverse_orientation =
        vec4f(
            -in.orientation.xyz,
            in.orientation.w,
        );

    return OrientedRay(
        rotate_vector(
            inverse_orientation,
            ray.origin - in.world_center,
        ),

        rotate_vector(
            inverse_orientation,
            ray.direction,
        ),

        in.orientation,

        in.size * 0.5,
    );
}
