enable clip_distances;

// Indexed transport-frame cartoon geometry into the shared gbuffer.
//
// Vertex/index fetching uses the fixed-function indexed path.
// User clip planes are emitted as hardware clip distances, removing clipping
// transforms, loops and discards from the fragment hot path.
//
// Face filtering is configured through the render pipeline cullMode.

//!include "include/camera.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/motion.wgsl"

const MAX_CLIP_PLANES: u32 = 4u;

struct CartoonVertexIn {
    @location(0) position: vec3f,
    @location(1) entity_id: u32,
    @location(2) normal: vec3f,
    @location(3) color: u32,
}

struct ModelUniforms {
    model_to_world: mat4x4f,
    world_to_model: mat4x4f,
    previous_model_to_world: mat4x4f,
    structure_id: u32,
    padding_a: u32,
    padding_b: u32,
    padding_c: u32,
}

struct ClipUniforms {
    planes: array<vec4f, 4>,
    metadata: vec4u,
    material: vec4f,
}

@group(2) @binding(2) var<uniform> model: ModelUniforms;
@group(2) @binding(3) var<uniform> clipping: ClipUniforms;

struct CartoonVsOut {
    @builtin(position) position: vec4f,
    @builtin(clip_distances) clip_distances: array<f32, 4>,

    // xyz = view normal, w = curve parameter.
    @location(0) normal_curve: vec4f,

    @location(1) color: vec4f,
    @location(2) view_position: vec3f,
    @location(3) motion: vec2f,
    @location(4) @interpolate(flat) entity_id: u32,
}

struct CartoonFsIn {
    @builtin(position) position: vec4f,

    // xyz = view normal, w = curve parameter.
    @location(0) normal_curve: vec4f,

    @location(1) color: vec4f,
    @location(2) view_position: vec3f,
    @location(3) motion: vec2f,
    @location(4) @interpolate(flat) entity_id: u32,
}

struct CartoonFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) structure_id: u32,
    @location(4) motion: vec2f,
}

/// Applies an affine transform without computing the unused homogeneous W.
fn transform_point(
    matrix: mat4x4f,
    position: vec3f,
) -> vec3f {
    return matrix[0].xyz * position.x
        + matrix[1].xyz * position.y
        + matrix[2].xyz * position.z
        + matrix[3].xyz;
}

/// Correctly transforms a model-space normal into view space.
///
/// Uses inverse-transpose model transformation and normalizes only after the
/// complete model-to-view transformation.
fn cartoon_view_normal(normal: vec3f) -> vec3f {
    let inverse_model = mat3x3f(
        model.world_to_model[0].xyz,
        model.world_to_model[1].xyz,
        model.world_to_model[2].xyz,
    );

    let world_normal =
        transpose(inverse_model) * normal;

    let view_normal =
        frame.view[0].xyz * world_normal.x
        + frame.view[1].xyz * world_normal.y
        + frame.view[2].xyz * world_normal.z;

    return normalize(view_normal);
}

/// Computes fixed-function clipping distances for active world-space planes.
fn cartoon_clip_distances(
    world_position: vec3f,
) -> array<f32, 4> {
    var distances = array<f32, 4>(
        1.0,
        1.0,
        1.0,
        1.0,
    );

    let count =
        min(clipping.metadata.x, MAX_CLIP_PLANES);

    for (var index = 0u; index < count; index++) {
        let plane = clipping.planes[index];

        distances[index] =
            dot(
                plane.xyz,
                world_position,
            ) + plane.w;
    }

    return distances;
}

@vertex
fn vs_cartoon(
    vertex: CartoonVertexIn,
    @builtin(vertex_index) vertex_index: u32,
) -> CartoonVsOut {
    let world_position =
        transform_point(
            model.model_to_world,
            vertex.position,
        );

    let view_position =
        transform_point(
            frame.view,
            world_position,
        );

    let previous_world_position =
        transform_point(
            model.previous_model_to_world,
            vertex.position,
        );

    var out: CartoonVsOut;

    out.position =
        frame.view_proj *
        vec4f(world_position, 1.0);

    out.clip_distances =
        cartoon_clip_distances(
            world_position,
        );

    out.normal_curve =
        vec4f(
            cartoon_view_normal(vertex.normal),
            f32(vertex_index >> 3u),
        );

    out.color =
        unpack4x8unorm(vertex.color);

    out.view_position =
        view_position;

    out.motion =
        screen_motion(
            world_position,
            previous_world_position,
        );

    out.entity_id =
        vertex.entity_id;

    return out;
}

@fragment
fn fs_cartoon(
    in: CartoonFsIn,
) -> CartoonFsOut {
    let normal =
        normalize(in.normal_curve.xyz);

    let tangent =
        curve_tangent(
            in.view_position,
            in.normal_curve.w,
            normal,
        );

    let material =
        ribbon_material_payload(
            clipping.material,
        );

    var out: CartoonFsOut;

    out.albedo_material =
        vec4f(
            in.color.rgb,
            material,
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                normal,
                tangent,
            ),
            clipping.material.x,
        );

    out.entity_id =
        in.entity_id;

    out.structure_id =
        model.structure_id;

    out.motion =
        in.motion;

    return out;
}

@fragment
fn fs_cartoon_transparent(
    in: CartoonFsIn,
) -> OitOutput {
    let normal =
        normalize(in.normal_curve.xyz);

    let tangent =
        curve_tangent(
            in.view_position,
            in.normal_curve.w,
            normal,
        );

    let material =
        ribbon_material_payload(
            clipping.material,
        );

    let lit =
        shade_ribbon(
            in.color.rgb,
            normal,
            tangent,
            clipping.material.x,
            material,
            in.view_position,
            oit_occlusion(in.position),
        );

    return weighted_transparency(
        lit,
        in.color.a,
        in.position.z,
    );
}
