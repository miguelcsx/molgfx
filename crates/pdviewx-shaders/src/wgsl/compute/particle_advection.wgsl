// Fixed-step, caller-sampled visual particle advection.
//
// Motion keeps the existing 64-byte layout while using the previously unused
// float slots to cache inverse boundary spans.
//
// previous.xyz preserves the caller-sampled position; previous.w stores age.

const MIN_SPAN: f32 = 1e-6;
const HASH_INDEX: u32 = 0x9e3779b9u;
const HASH_CHANNEL: u32 = 0x85ebca6bu;
const HASH_MIX_A: u32 = 0x7feb352du;
const HASH_MIX_B: u32 = 0x846ca68bu;
const HASH_MASK_24: u32 = 0x00ffffffu;
const UNIT_24: f32 = 16777215.0;

struct PrimitiveGpu {
    center_radius: vec4f,
    orientation: vec4f,
    size_opacity: vec4f,
    inverse_primary: vec4f,
    inverse_cross: vec4f,
    color: vec4f,
    metadata: vec4u,
}

struct ParticleMotionGpu {
    velocity: vec3f,
    inverse_span_x: f32,
    minimum: vec3f,
    inverse_span_y: f32,
    maximum: vec3f,
    inverse_span_z: f32,
    metadata: vec4u,
}

@group(2) @binding(0) var<storage, read_write> primitive: array<PrimitiveGpu>;
@group(2) @binding(1) var<storage, read_write> previous: array<vec4f>;
@group(2) @binding(2) var<storage, read> motion: array<ParticleMotionGpu>;

/// Wraps a position into its presentation bounds.
fn wrap_position(
    value: vec3f,
    minimum: vec3f,
    maximum: vec3f,
    inverse_span: vec3f,
) -> vec3f {
    let span = max(maximum - minimum, vec3f(MIN_SPAN));
    return minimum + fract((value - minimum) * inverse_span) * span;
}

/// Reflects a position through its presentation bounds.
fn bounce_position(
    value: vec3f,
    minimum: vec3f,
    maximum: vec3f,
    inverse_span: vec3f,
) -> vec3f {
    let span = max(maximum - minimum, vec3f(MIN_SPAN));
    let cycle = fract((value - minimum) * inverse_span * 0.5) * 2.0;

    return minimum
        + (vec3f(1.0) - abs(cycle - vec3f(1.0))) * span;
}

/// Produces the same three 24-bit hash channels in parallel.
fn particle_unit3(seed: u32, index: u32) -> vec3f {
    let base = seed ^ ((index + 1u) * HASH_INDEX);

    var value =
        vec3u(base) ^
        (vec3u(1u, 2u, 3u) * vec3u(HASH_CHANNEL));

    value =
        (value ^ (value >> vec3u(16u))) *
        vec3u(HASH_MIX_A);

    value =
        (value ^ (value >> vec3u(15u))) *
        vec3u(HASH_MIX_B);

    value ^= value >> vec3u(16u);

    return vec3f(value & vec3u(HASH_MASK_24))
        / vec3f(UNIT_24);
}

/// Generates a deterministic respawn position.
fn respawn_position(
    minimum: vec3f,
    maximum: vec3f,
    seed: u32,
    index: u32,
) -> vec3f {
    return minimum
        + (maximum - minimum) * particle_unit3(seed, index);
}

@compute @workgroup_size(64)
fn advect_particles(@builtin(global_invocation_id) id: vec3u) {
    let index = id.x;

    if index >= arrayLength(&motion) {
        return;
    }

    let metadata = motion[index].metadata;
    let center_radius = primitive[index].center_radius;
    let current = center_radius.xyz;

    if metadata.x == 0u {
        previous[index] = vec4f(current, 0.0);
        return;
    }

    let minimum = motion[index].minimum;
    let maximum = motion[index].maximum;

    var age = previous[index].w + 1.0;
    var next: vec3f;

    if metadata.w != 0u && age >= f32(metadata.w) {
        next = respawn_position(
            minimum,
            maximum,
            metadata.z,
            index,
        );
        age = 0.0;
    } else {
        let candidate = current + motion[index].velocity;

        let inverse_span = vec3f(
            motion[index].inverse_span_x,
            motion[index].inverse_span_y,
            motion[index].inverse_span_z,
        );

        if metadata.y == 0u {
            next = bounce_position(
                candidate,
                minimum,
                maximum,
                inverse_span,
            );
        } else {
            next = wrap_position(
                candidate,
                minimum,
                maximum,
                inverse_span,
            );
        }
    }

    previous[index] = vec4f(current, age);
    primitive[index].center_radius = vec4f(next, center_radius.w);
}
