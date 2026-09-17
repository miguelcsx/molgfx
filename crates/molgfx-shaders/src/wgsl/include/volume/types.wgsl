// Volume records, bindings and proxy-quad transforms.
//
// The rendering mode is a pipeline constant, so a draw carries one algorithm
// rather than a switch evaluated on every sample of every ray.

const VOLUME_RENDER_ISOSURFACE: u32 = 1u;
const VOLUME_RENDER_MEDIUM: u32 = 2u;
const VOLUME_RENDER_SLICE: u32 = 3u;
const VOLUME_RENDER_LIQUID: u32 = 4u;
const VOLUME_RENDER_DIRECT: u32 = 0u;

override VOLUME_RENDER_MODE: u32 = VOLUME_RENDER_DIRECT;
override VOLUME_CLIPPING_ENABLED: bool = true;

const VOLUME_RAY_EPSILON: f32 = 1.0e-7;
const VOLUME_DISTANCE_INFINITY: f32 = 1.0e20;
const VOLUME_MIN_STEP: f32 = 1.0e-4;
const VOLUME_OPACITY_EPSILON: f32 = 1.0e-4;
const VOLUME_TERMINATION_ALPHA: f32 = 0.985;
const VOLUME_REPRESENTATIVE_ALPHA: f32 = 0.18;
const VOLUME_HASH_SCALE: f32 = 1.0 / 16777216.0;

const MEDIUM_LIGHT_VIEW: vec3f =
    vec3f(-0.41941323, 0.57918970, 0.69902205);

struct VolumeUniforms {
    voxel_to_world: mat4x4f,
    world_to_voxel: mat4x4f,
    dimensions: vec4u,
    empty_space_dimensions: vec4u,
    scalar: vec4f,
    sampling: vec4f,
    transfer_values: array<vec4f, 8>,
    transfer_colors: array<vec4f, 8>,
    transfer_meta: vec4u,
    clip_planes: array<vec4f, 4>,
    clip_meta: vec4u,
    crop_minimum: vec4u,
    crop_maximum: vec4u,
    slice_plane: vec4f,
    material: vec4f,
}

@group(2) @binding(0) var density_texture: texture_3d<f32>;
@group(2) @binding(1) var<uniform> volume: VolumeUniforms;
@group(2) @binding(2) var empty_space_bounds_texture: texture_3d<f32>;

struct VolumeVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(linear) world_origin: vec3f,
    @location(1) @interpolate(linear) world_vector: vec3f,

    @location(2) @interpolate(linear) voxel_origin: vec3f,
    @location(3) @interpolate(linear) voxel_vector: vec3f,

    // x = near-view Z, y = far-view Z - near-view Z.
    @location(4) @interpolate(linear) view_z: vec2f,
}

struct VolumeRay {
    world_origin: vec3f,
    world_direction: vec3f,

    voxel_origin: vec3f,
    voxel_direction: vec3f,
    voxel_inverse_direction: vec3f,

    view_origin_z: f32,
    view_direction_z: f32,
}

struct DensityCell {
    lower: vec3u,
    fraction: vec3f,
}

struct DensityCorners {
    c000: f32,
    c100: f32,
    c010: f32,
    c110: f32,
    c001: f32,
    c101: f32,
    c011: f32,
    c111: f32,
}

struct TransferSample {
    color: vec3f,
    opacity: f32,
}

struct EmptySpaceCell {
    minimum: f32,
    maximum: f32,
    exit_distance: f32,
}

struct MediumState {
    light_step: vec3f,
    phase_scale: f32,
    sample_count: u32,
}

/// Applies an affine point transform.
fn volume_transform_point(
    transform: mat4x4f,
    point: vec3f,
) -> vec3f {
    return transform[0].xyz * point.x
        + transform[1].xyz * point.y
        + transform[2].xyz * point.z
        + transform[3].xyz;
}

/// Applies only the linear portion of a transform.
fn volume_transform_direction(
    transform: mat4x4f,
    direction: vec3f,
) -> vec3f {
    return transform[0].xyz * direction.x
        + transform[1].xyz * direction.y
        + transform[2].xyz * direction.z;
}

fn volume_homogeneous_point(value: vec4f) -> vec3f {
    return value.xyz *
        (
            sign(value.w) /
            max(abs(value.w), VOLUME_RAY_EPSILON)
        );
}

fn volume_quad_uv(vertex: u32) -> vec2f {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return corners[min(vertex, 5u)];
}

/// Computes conservative NDC bounds of the transformed volume.
fn volume_ndc_bounds() -> mat2x2f {
    let upper =
        vec3f(volume.dimensions.xyz - vec3u(1u));

    var low = vec2f(1.0);
    var high = vec2f(-1.0);

    for (var corner = 0u; corner < 8u; corner++) {
        let selector =
            vec3f(
                f32(corner & 1u),
                f32((corner >> 1u) & 1u),
                f32((corner >> 2u) & 1u),
            );

        let world =
            volume_transform_point(
                volume.voxel_to_world,
                upper * selector,
            );

        let clip =
            frame.view_proj *
            vec4f(world, 1.0);

        if clip.w <= 0.0 {
            return mat2x2f(
                vec2f(-1.0),
                vec2f(1.0),
            );
        }

        let ndc =
            clip.xy *
            (1.0 / clip.w);

        low = min(low, ndc);
        high = max(high, ndc);
    }

    return mat2x2f(
        clamp(low, vec2f(-1.0), vec2f(1.0)),
        clamp(high, vec2f(-1.0), vec2f(1.0)),
    );
}
