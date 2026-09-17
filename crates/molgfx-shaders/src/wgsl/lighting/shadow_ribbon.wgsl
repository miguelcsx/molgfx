// Depth-only cartoon-ribbon shadow caster.
//
// The atom and primitive shadow casters share one module built on the atom
// bindings, which claim the storage slots a ribbon needs for its vertex and
// index buffers. The ribbon therefore casts from its own module: it pulls the
// mesh from storage by vertex index, matching the engine's meshless indirect
// draw path instead of the fixed-function vertex-buffer path the hardware
// abstraction does not expose.

//!include "include/camera.wgsl"

struct ShadowRibbonModel {
    model_to_world: mat4x4f,
    world_to_model: mat4x4f,
    previous_model_to_world: mat4x4f,
    pick_pages_a: vec4u,
    pick_pages_b: vec4u,
    pick_pages_c: vec4u,
}

// Eight cross-section vertices per spline sample; the same 32-byte record the
// cartoon pass draws, shared through one bind group.
struct ShadowRibbonVertex {
    position: vec3f,
    entity_id: u32,
    normal: vec3f,
    color: u32,
}

//!include "include/ribbon/deform.wgsl"

@group(2) @binding(0) var<storage, read> shadow_ribbon_vertices: array<ShadowRibbonVertex>;
@group(2) @binding(1) var<storage, read> shadow_ribbon_indices: array<u32>;
@group(2) @binding(2) var<uniform> shadow_ribbon_model: ShadowRibbonModel;

//!include "include/visual/ribbon_shadow.wgsl"

struct ShadowRibbonOut {
    @builtin(position) position: vec4f,
    @location(0) local_position: vec3f,
    @location(1) world_position: vec3f,
    @location(2) world_normal: vec3f,
    @location(3) color: vec4f,
    @location(4) @interpolate(flat) entity_id: u32,
}

@vertex
fn vs_shadow_ribbon(
    @builtin(vertex_index) draw_index: u32,
) -> ShadowRibbonOut {
    let vertex_id = shadow_ribbon_indices[draw_index];
    let vertex = shadow_ribbon_vertices[vertex_id];
    let dynamic = ribbon_deform(vertex_id, vertex.position, vertex.normal, false);
    let position = select(vertex.position, dynamic.position, dynamic.enabled != 0u)
        + ribbon_shadow_offset(vertex.entity_id);
    let normal = select(vertex.normal, dynamic.normal, dynamic.enabled != 0u);

    let world =
        shadow_ribbon_model.model_to_world *
        vec4f(position, 1.0);

    let inverse_model = mat3x3f(
        shadow_ribbon_model.world_to_model[0].xyz,
        shadow_ribbon_model.world_to_model[1].xyz,
        shadow_ribbon_model.world_to_model[2].xyz,
    );
    return ShadowRibbonOut(
        frame.shadow_view_proj * world,
        position,
        world.xyz,
        normalize(transpose(inverse_model) * normal),
        unpack4x8unorm(vertex.color),
        vertex.entity_id,
    );
}

@fragment
fn fs_shadow_ribbon(in: ShadowRibbonOut) {
    if !ribbon_shadow_visible(
        in.entity_id,
        in.color,
        in.local_position,
        in.world_position,
        in.world_normal,
    ) {
        discard;
    }
}
