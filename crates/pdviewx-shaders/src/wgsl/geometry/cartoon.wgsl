// Indexed transport-frame cartoon geometry into the shared gbuffer.
//
// Vertex/index fetching uses the fixed-function indexed path.
// User clip planes are evaluated per vertex and discarded analytically in the
// fragment stage, matching every other representation and keeping the shader
// portable to adapters without the hardware clip-distance extension.
//
// Face filtering is configured through the render pipeline cullMode.

//!include "include/camera.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/motion.wgsl"

const MAX_CLIP_PLANES: u32 = 4u;

// Eight cross-section vertices per spline sample, so an original vertex index
// shifted right by three is its sample — the curve parameter along the ribbon.
const PROFILE_SIDES_SHIFT: u32 = 3u;

// The ribbon mesh is pulled from storage rather than fed as vertex-buffer
// attributes: the engine draws by vertex index into these buffers, so the same
// indirect, meshless draw path serves ribbons as serves impostors.
struct RibbonVertex {
    position: vec3f,
    entity_id: u32,
    normal: vec3f,
    color: u32,
}

//!include "include/ribbon/deform.wgsl"

@group(2) @binding(0) var<storage, read> ribbon_vertices: array<RibbonVertex>;
@group(2) @binding(1) var<storage, read> ribbon_indices: array<u32>;

struct ModelUniforms {
    model_to_world: mat4x4f,
    world_to_model: mat4x4f,
    previous_model_to_world: mat4x4f,
    pick_pages_a: vec4u,
    pick_pages_b: vec4u,
    pick_pages_c: vec4u,
}

@group(2) @binding(2) var<uniform> model: ModelUniforms;

fn cartoon_pick_local_row(entity_id: u32) -> u32 {
    return entity_id & 0x0fffffffu;
}

fn cartoon_pick_page(entity_id: u32) -> u32 {
    let kind = entity_id >> 28u;
    if kind < 4u {
        return model.pick_pages_a[kind];
    }
    if kind < 8u {
        return model.pick_pages_b[kind - 4u];
    }
    return model.pick_pages_c[kind - 8u];
}
//!include "include/visual/ribbon.wgsl"

struct CartoonVsOut {
    @builtin(position) position: vec4f,

    // xyz = view normal, w = curve parameter.
    @location(0) normal_curve: vec4f,

    @location(1) color: vec4f,
    @location(2) view_position: vec3f,
    @location(3) motion: vec2f,
    @location(4) @interpolate(flat) entity_id: u32,

    // Signed distance to each clip plane; an inactive plane stays positive.
    @location(5) clip: vec4f,
}

struct CartoonFsIn {
    @builtin(position) position: vec4f,

    // xyz = view normal, w = curve parameter.
    @location(0) normal_curve: vec4f,

    @location(1) color: vec4f,
    @location(2) view_position: vec3f,
    @location(3) motion: vec2f,
    @location(4) @interpolate(flat) entity_id: u32,

    @location(5) clip: vec4f,
}

struct CartoonFsOut {
    @location(0) albedo_material: vec4f,
    @location(1) normal_roughness: vec4f,
    @location(2) entity_id: u32,
    @location(3) resident_page: u32,
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

/// Signed distance to each active world-space clip plane. An inactive plane
/// stays at +1 so it never clips, letting the fragment test all four without
/// knowing the active count.
fn cartoon_clip_distances(
    world_position: vec3f,
) -> vec4f {
    var distances = vec4f(1.0);

    let count =
        min(ribbon_uniforms.metadata.x, MAX_CLIP_PLANES);

    for (var index = 0u; index < count; index++) {
        let plane = ribbon_uniforms.planes[index];

        distances[index] =
            dot(
                plane.xyz,
                world_position,
            ) + plane.w;
    }

    return distances;
}

/// True when a fragment lies outside any active clip plane. Inactive planes
/// hold +1, so their lanes never fail the test.
fn cartoon_clipped(clip: vec4f) -> bool {
    return min(min(clip.x, clip.y), min(clip.z, clip.w)) < 0.0;
}

@vertex
fn vs_cartoon(
    @builtin(vertex_index) draw_index: u32,
) -> CartoonVsOut {
    // Manual indexed fetch: the draw index addresses the index buffer, which
    // names the original vertex. That original index carries the ring layout,
    // so its high bits are the spline sample used as the curve parameter.
    let vertex_id = ribbon_indices[draw_index];
    let vertex = ribbon_vertices[vertex_id];
    let current = ribbon_deform(vertex_id, vertex.position, vertex.normal, false);
    let previous = ribbon_deform(vertex_id, vertex.position, vertex.normal, true);
    let model_position = select(vertex.position, current.position, current.enabled != 0u)
        + ribbon_visual_offset(vertex.entity_id);
    let model_normal = select(vertex.normal, current.normal, current.enabled != 0u);
    let previous_model_position = select(
        vertex.position,
        previous.position,
        previous.enabled != 0u,
    );

    let world_position =
        transform_point(
            model.model_to_world,
            model_position,
        );

    let view_position =
        transform_point(
            frame.view,
            world_position,
        );

    let previous_world_position =
        transform_point(
            model.previous_model_to_world,
            previous_model_position,
        );

    var out: CartoonVsOut;

    out.position =
        frame.view_proj *
        vec4f(world_position, 1.0);

    out.clip =
        cartoon_clip_distances(
            world_position,
        );

    out.normal_curve =
        vec4f(
            cartoon_view_normal(model_normal),
            f32(vertex_id >> PROFILE_SIDES_SHIFT),
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
    if cartoon_clipped(in.clip) {
        discard;
    }

    let normal =
        normalize(in.normal_curve.xyz);

    let tangent =
        curve_tangent(
            in.view_position,
            in.normal_curve.w,
            normal,
        );

    let world_position = transform_point(frame.inv_view, in.view_position);
    let world_normal = normalize(
        frame.inv_view[0].xyz * normal.x
            + frame.inv_view[1].xyz * normal.y
            + frame.inv_view[2].xyz * normal.z
    );
    let visual = ribbon_visual(
        in.entity_id,
        in.color,
        transform_point(model.world_to_model, world_position),
        world_position,
        world_normal,
    );
    if !visual.visible {
        discard;
    }

    var out: CartoonFsOut;

    out.albedo_material =
        vec4f(
            visual.color.rgb + visual.emission,
            ribbon_visual_gbuffer_material(visual),
        );

    out.normal_roughness =
        vec4f(
            encode_shading_frame(
                normal,
                tangent,
            ),
            visual.roughness,
        );

    out.entity_id =
        cartoon_pick_local_row(in.entity_id);

    out.resident_page =
        cartoon_pick_page(in.entity_id);

    out.motion =
        in.motion;

    return out;
}

@fragment
fn fs_cartoon_transparent(
    in: CartoonFsIn,
) -> OitOutput {
    if cartoon_clipped(in.clip) {
        discard;
    }

    let normal =
        normalize(in.normal_curve.xyz);

    let tangent =
        curve_tangent(
            in.view_position,
            in.normal_curve.w,
            normal,
        );

    let world_position = transform_point(frame.inv_view, in.view_position);
    let world_normal = normalize(
        frame.inv_view[0].xyz * normal.x
            + frame.inv_view[1].xyz * normal.y
            + frame.inv_view[2].xyz * normal.z
    );
    let visual = ribbon_visual(
        in.entity_id,
        in.color,
        transform_point(model.world_to_model, world_position),
        world_position,
        world_normal,
    );
    if !visual.visible {
        discard;
    }

    let lit =
        shade_ribbon(
            visual.color.rgb,
            normal,
            tangent,
            visual.roughness,
            ribbon_visual_material(visual),
            in.view_position,
            oit_occlusion(in.position),
        ) + visual.emission;

    return weighted_transparency(
        lit,
        visual.color.a,
        in.position.z,
    );
}
