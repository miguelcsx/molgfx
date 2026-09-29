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

fn cartoon_pick_local_row(
    entity_id: u32,
) -> u32 {
    return entity_id & 0x0fffffffu;
}

fn cartoon_pick_page(
    entity_id: u32,
) -> u32 {
    let kind =
        entity_id >> 28u;

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
    @location(4) @interpolate(flat, either) entity_id: u32,

    // Signed distance to each clip plane; an inactive plane stays positive.
    @location(5) clip: vec4f,
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
/// `world_to_model` is the inverse model transform. Multiplying the model
/// normal by its transpose is expressed directly as three dot products,
/// avoiding construction and transposition of a temporary mat3x3.
fn cartoon_view_normal(
    normal: vec3f,
) -> vec3f {
    let world_normal =
        vec3f(
            dot(
                model.world_to_model[0].xyz,
                normal,
            ),
            dot(
                model.world_to_model[1].xyz,
                normal,
            ),
            dot(
                model.world_to_model[2].xyz,
                normal,
            ),
        );

    let view_normal =
        frame.view[0].xyz * world_normal.x
        + frame.view[1].xyz * world_normal.y
        + frame.view[2].xyz * world_normal.z;

    return normalize(
        view_normal,
    );
}

/// Transforms a normalized view-space normal back into world space.
fn cartoon_world_normal(
    view_normal: vec3f,
) -> vec3f {
    let world_normal =
        frame.inv_view[0].xyz * view_normal.x
        + frame.inv_view[1].xyz * view_normal.y
        + frame.inv_view[2].xyz * view_normal.z;

    return normalize(
        world_normal,
    );
}

/// Signed distance to each active world-space clip plane. An inactive plane
/// stays at +1 so it never clips.
///
/// MAX_CLIP_PLANES is four, so spelling out the bounded cases removes the
/// loop counter, dynamic plane indexing and dynamic vector-component write.
/// `count` is uniform for the draw, so these exits are coherent.
fn cartoon_clip_distances(
    world_position: vec3f,
) -> vec4f {
    let count =
        min(
            ribbon_uniforms.metadata.x,
            MAX_CLIP_PLANES,
        );

    var distances =
        vec4f(1.0);

    if count == 0u {
        return distances;
    }

    let plane_0 =
        ribbon_uniforms.planes[0];

    distances.x =
        dot(
            plane_0.xyz,
            world_position,
        ) + plane_0.w;

    if count == 1u {
        return distances;
    }

    let plane_1 =
        ribbon_uniforms.planes[1];

    distances.y =
        dot(
            plane_1.xyz,
            world_position,
        ) + plane_1.w;

    if count == 2u {
        return distances;
    }

    let plane_2 =
        ribbon_uniforms.planes[2];

    distances.z =
        dot(
            plane_2.xyz,
            world_position,
        ) + plane_2.w;

    if count == 3u {
        return distances;
    }

    let plane_3 =
        ribbon_uniforms.planes[3];

    distances.w =
        dot(
            plane_3.xyz,
            world_position,
        ) + plane_3.w;

    return distances;
}


@vertex
fn vs_cartoon(
    @builtin(vertex_index) draw_index: u32,
) -> CartoonVsOut {
    // Manual indexed fetch: the draw index addresses the index buffer, which
    // names the original vertex. That original index carries the ring layout,
    // so its high bits are the spline sample used as the curve parameter.
    let vertex_id =
        ribbon_indices[draw_index];

    let vertex =
        ribbon_vertices[vertex_id];

    let entity_id =
        vertex.entity_id;

    let base_position =
        vertex.position;

    let base_normal =
        vertex.normal;

    let current =
        ribbon_deform(
            vertex_id,
            base_position,
            base_normal,
            false,
        );

    let previous =
        ribbon_deform(
            vertex_id,
            base_position,
            base_normal,
            true,
        );

    let current_enabled =
        current.enabled != 0u;

    let previous_enabled =
        previous.enabled != 0u;

    let model_position =
        select(
            base_position,
            current.position,
            current_enabled,
        ) + ribbon_visual_offset(
            entity_id,
        );

    let model_normal =
        select(
            base_normal,
            current.normal,
            current_enabled,
        );

    let previous_model_position =
        select(
            base_position,
            previous.position,
            previous_enabled,
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
        frame.view_proj
        * vec4f(
            world_position,
            1.0,
        );

    out.clip =
        cartoon_clip_distances(
            world_position,
        );

    out.normal_curve =
        vec4f(
            cartoon_view_normal(
                model_normal,
            ),
            f32(
                vertex_id
                >> PROFILE_SIDES_SHIFT,
            ),
        );

    out.color =
        unpack4x8unorm(
            vertex.color,
        );

    out.view_position =
        view_position;

    out.motion =
        screen_motion(
            world_position,
            previous_world_position,
        );

    out.entity_id =
        entity_id;

    return out;
}

//!include "include/visual/cartoon.wgsl"
