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
    structure_id: u32,
    padding_a: u32,
    padding_b: u32,
    padding_c: u32,
}

// Eight cross-section vertices per spline sample; the same 32-byte record the
// cartoon pass draws, shared through one bind group.
struct ShadowRibbonVertex {
    position: vec3f,
    entity_id: u32,
    normal: vec3f,
    color: u32,
}

@group(2) @binding(0) var<storage, read> shadow_ribbon_vertices: array<ShadowRibbonVertex>;
@group(2) @binding(1) var<storage, read> shadow_ribbon_indices: array<u32>;
@group(2) @binding(2) var<uniform> shadow_ribbon_model: ShadowRibbonModel;

@vertex
fn vs_shadow_ribbon(
    @builtin(vertex_index) draw_index: u32,
) -> @builtin(position) vec4f {
    let vertex =
        shadow_ribbon_vertices[
            shadow_ribbon_indices[draw_index]
        ];

    let world =
        shadow_ribbon_model.model_to_world *
        vec4f(vertex.position, 1.0);

    return frame.shadow_view_proj *
        world;
}
