// Shadow caster records and light-space setup.
//
// The shadow projection is orthographic, so every ray is light-space -Z and a
// hit's depth follows from its parameter alone. Solving in that frame is what
// lets each caster skip a general ray transform per fragment.

const SHADOW_DIRECTION_EPSILON: f32 = 1.0e-7;
const SHADOW_QUADRATIC_EPSILON: f32 = 1.0e-7;

const SHADOW_KIND_ELLIPSOID: u32 = 0u;
const SHADOW_KIND_BOX: u32 = 1u;
const SHADOW_KIND_PARTICLE_SPHERE: u32 = 2u;
const SHADOW_KIND_PARTICLE_CYLINDER: u32 = 3u;
const SHADOW_KIND_PARTICLE_SPHEROCYLINDER: u32 = 4u;
const SHADOW_KIND_POLYGON_PENTAGON: u32 = 5u;
const SHADOW_KIND_POLYGON_HEXAGON: u32 = 6u;
const SHADOW_KIND_PARTICLE_CIRCLE: u32 = 7u;
const SHADOW_KIND_PARTICLE_SQUARE: u32 = 8u;
const SHADOW_KIND_PARTICLE_SUPERQUADRIC: u32 = 9u;

// Create one primitive pipeline per kind.
override SHADOW_PRIMITIVE_KIND: u32 = SHADOW_KIND_ELLIPSOID;

struct ShadowPrimitiveGpu {
    center_radius: vec4f,
    orientation: vec4f,
    size_opacity: vec4f,
    inverse_primary: vec4f,
    inverse_cross: vec4f,
    color: vec4f,
    metadata: vec4u,
}

// Binding 0..5 and 13 are owned by atom.wgsl.
@group(2) @binding(6)
var<storage, read> shadow_primitives: array<ShadowPrimitiveGpu>;

// -----------------------------------------------------------------------------
// Shared helpers
// -----------------------------------------------------------------------------

/// Returns true for the provoking vertices of the two independent triangles.
fn shadow_flat_source(vertex: u32) -> bool {
    return vertex == 0u || vertex == 3u;
}

/// Transforms a world-space point into directional-light view space.
fn shadow_view_position(world: vec3f) -> vec3f {
    return frame.shadow_view[0].xyz * world.x
        + frame.shadow_view[1].xyz * world.y
        + frame.shadow_view[2].xyz * world.z
        + frame.shadow_view[3].xyz;
}

/// Reconstructs a world-space point on the light-space Z=0 plane.
fn shadow_world_origin(light_xy: vec2f) -> vec3f {
    return frame.shadow_inv_view[0].xyz * light_xy.x
        + frame.shadow_inv_view[1].xyz * light_xy.y
        + frame.shadow_inv_view[3].xyz;
}

/// World-space direction of the orthographic shadow ray.
///
/// shadow_inv_view is rigid, so this direction is already unit length.
fn shadow_world_direction() -> vec3f {
    return -frame.shadow_inv_view[2].xyz;
}

fn shadow_world_position(light_position: vec3f) -> vec3f {
    return frame.shadow_inv_view[0].xyz * light_position.x
        + frame.shadow_inv_view[1].xyz * light_position.y
        + frame.shadow_inv_view[2].xyz * light_position.z
        + frame.shadow_inv_view[3].xyz;
}

/// Projects a light-view position.
fn shadow_clip(light_position: vec3f) -> vec4f {
    return frame.shadow_projection *
        vec4f(light_position, 1.0);
}

/// Computes only projected Z/W for the final analytic hit.
fn shadow_depth(light_position: vec3f) -> f32 {
    let zw =
        frame.shadow_projection[0].zw * light_position.x
        + frame.shadow_projection[1].zw * light_position.y
        + frame.shadow_projection[2].zw * light_position.z
        + frame.shadow_projection[3].zw;

    return zw.x / zw.y;
}

/// Rotates a vector by a caller-normalized quaternion.
fn shadow_rotate(
    quaternion: vec4f,
    value: vec3f,
) -> vec3f {
    let twice_cross =
        2.0 * cross(quaternion.xyz, value);

    return value
        + quaternion.w * twice_cross
        + cross(quaternion.xyz, twice_cross);
}

/// Applies the symmetric inverse ellipsoid tensor without a temporary matrix.
fn shadow_inverse_tensor_apply(
    primary: vec4f,
    cross_terms: vec4f,
    value: vec3f,
) -> vec3f {
    return vec3f(
        primary.x * value.x
            + primary.w * value.y
            + cross_terms.x * value.z,

        primary.w * value.x
            + primary.y * value.y
            + cross_terms.y * value.z,

        cross_terms.x * value.x
            + cross_terms.y * value.y
            + primary.z * value.z,
    );
}
