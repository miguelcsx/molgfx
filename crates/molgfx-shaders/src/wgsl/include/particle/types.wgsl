// Particle shape identifiers, local frames and shared helpers.
//
// A particle is oriented by a unit quaternion rather than a stored matrix, so
// an instance carries four floats instead of nine and the rotation preserves
// ray and normal length without a renormalize. The shape itself is a pipeline
// override constant, so a draw resolves one shape with no per-fragment branch.

const PARTICLE_SHAPE_SPHERE: u32 = 0u;
const PARTICLE_SHAPE_CYLINDER: u32 = 2u;
const PARTICLE_SHAPE_SPHEROCYLINDER: u32 = 3u;
const PARTICLE_SHAPE_GAUSSIAN: u32 = 4u;
const PARTICLE_SHAPE_CIRCLE: u32 = 5u;
const PARTICLE_SHAPE_SQUARE: u32 = 6u;
const PARTICLE_SHAPE_SUPERQUADRIC: u32 = 7u;

// One specialized pipeline per shape. metadata.w is no longer consulted.
override PARTICLE_SHAPE_KIND: u32 = PARTICLE_SHAPE_SPHERE;

const PARTICLE_RAY_EPSILON: f32 = 1.0e-7;
const PARTICLE_SIZE_EPSILON: f32 = 1.0e-5;
const PARTICLE_GAUSSIAN_EPSILON: f32 = 1.0e-4;
const PARTICLE_GAUSSIAN_INV_SIGMA_SCALE: f32 = 1.0 / 6.0;

const SUPERQUADRIC_STEPS: u32 = 64u;
const SUPERQUADRIC_REFINEMENTS: u32 = 12u;
const SUPERQUADRIC_INV_STEPS: f32 = 1.0 / 64.0;
const SUPERQUADRIC_GRADIENT_EPSILON: f32 = 1.0e-8;

struct ParticleLocalFrame {
    origin: vec3f,
    direction: vec3f,
}

struct ParticlePlaneHit {
    point: vec2f,
    t: f32,
    direction_z: f32,
    valid: bool,
}

struct SuperquadricParams {
    inv_half_size: vec3f,
    xy_power: f32,
    z_power: f32,
    radial_power: f32,
}

fn particle_miss() -> PrimitiveHit {
    return PrimitiveHit(
        -1.0,
        vec3f(0.0),
        0.0,
        false,
    );
}

/// Transforms a normalized world ray into particle-local space.
fn particle_local_frame(
    in: PrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> ParticleLocalFrame {
    let inverse_orientation =
        vec4f(
            -in.orientation.xyz,
            in.orientation.w,
        );

    return ParticleLocalFrame(
        rotate_vector(
            inverse_orientation,
            origin - in.world_center,
        ),
        rotate_vector(
            inverse_orientation,
            direction,
        ),
    );
}

/// Rotates a unit local normal back into world space.
fn particle_world_normal(
    in: PrimitiveVsOut,
    normal: vec3f,
) -> vec3f {
    return rotate_vector(
        in.orientation,
        normal,
    );
}

fn particle_inverse_radius(radius: f32) -> f32 {
    return 1.0 /
        max(
            abs(radius),
            PARTICLE_SIZE_EPSILON,
        );
}
