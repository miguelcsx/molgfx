// Categorical volume records, styles and transforms.
//
// A label grid is caller-owned exact data, so the uniforms describe how to
// address it and never how to derive it. Slice mode, hashed lookup and
// clipping are pipeline constants, so a draw carries only the addressing it
// actually uses.

override SEGMENTATION_SLICE_MODE: bool = false;
override SEGMENTATION_HASH_LOOKUP: bool = false;
override SEGMENTATION_CLIPPING_ENABLED: bool = true;

const SEGMENT_RAY_EPSILON: f32 = 1.0e-7;
const SEGMENT_INFINITY: f32 = 1.0e20;
const SEGMENT_MIN_STEP: f32 = 1.0e-4;
const SEGMENT_OPACITY_EPSILON: f32 = 1.0e-4;
const SEGMENT_TERMINATION_ALPHA: f32 = 0.985;
const SEGMENT_REPRESENTATIVE_ALPHA: f32 = 0.18;
const SEGMENT_HASH_SCALE: f32 = 1.0 / 16777216.0;

struct SegmentationUniforms {
    voxel_to_world: mat4x4f,
    world_to_voxel: mat4x4f,
    dimensions: vec4u,
    sampling: vec4f,
    lookup: vec4u,
    clip_planes: array<vec4f, 4>,
    clip_meta: vec4u,
    crop_minimum: vec4u,
    crop_maximum: vec4u,
    slice_plane: vec4f,
    material: vec4f,
}

struct SegmentStyleGpu {
    label: u32,
    present: u32,
    opacity: f32,
    padding: u32,
    color: vec4f,
}

struct SegmentStyleSample {
    color: vec3f,
    opacity: f32,
    found: bool,
}

struct SegmentationVsOut {
    @builtin(position) position: vec4f,

    @location(0) @interpolate(linear) world_origin: vec3f,
    @location(1) @interpolate(linear) world_vector: vec3f,

    @location(2) @interpolate(linear) voxel_origin: vec3f,
    @location(3) @interpolate(linear) voxel_vector: vec3f,

    @location(4) @interpolate(linear) view_z: vec2f,
}

struct SegmentationRay {
    world_origin: vec3f,
    world_direction: vec3f,
    voxel_origin: vec3f,
    voxel_direction: vec3f,
    voxel_inverse_direction: vec3f,
    view_origin_z: f32,
    view_direction_z: f32,
}

struct SegmentationOutput {
    @location(0) accumulation: vec4f,
    @location(1) revealage: f32,
    @location(2) source_id: u32,
    @location(3) label: u32,
    @builtin(frag_depth) depth: f32,
}

@group(2) @binding(0) var label_texture: texture_3d<u32>;
@group(2) @binding(1) var<uniform> volume: SegmentationUniforms;
@group(2) @binding(2) var<storage, read> segment_styles: array<SegmentStyleGpu>;

fn segment_transform_point(
    transform: mat4x4f,
    point: vec3f,
) -> vec3f {
    return transform[0].xyz * point.x
        + transform[1].xyz * point.y
        + transform[2].xyz * point.z
        + transform[3].xyz;
}

fn segment_transform_direction(
    transform: mat4x4f,
    direction: vec3f,
) -> vec3f {
    return transform[0].xyz * direction.x
        + transform[1].xyz * direction.y
        + transform[2].xyz * direction.z;
}

fn segment_quad_uv(vertex: u32) -> vec2f {
    return vec2f(
        f32(vertex & 1u),
        f32(vertex >> 1u),
    );
}

fn segment_homogeneous(value: vec4f) -> vec3f {
    return value.xyz *
        (
            sign(value.w) /
            max(abs(value.w), SEGMENT_RAY_EPSILON)
        );
}

fn segmentation_bounds() -> mat2x2f {
    let upper =
        vec3f(
            volume.dimensions.xyz -
            vec3u(1u)
        );

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
            segment_transform_point(
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
