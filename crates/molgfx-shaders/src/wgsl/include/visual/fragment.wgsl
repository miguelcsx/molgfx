// Fragment-stage visual-program resolution.
//
// Entity and uniform programs use their compact precomputed result. Only a
// program that references shaded geometry or camera inputs enters the bounded
// interpreter below. Recomputing its at-most-64 values per styled fragment
// avoids retaining a 1 KiB register table for every particle in the scene.

override VISUAL_PROGRAM_ENABLED: bool = false;
override VISUAL_FRAGMENT_ENABLED: bool = false;

struct VisualFragmentProgram {
    instructions: array<VisualInstruction, 64>,
    parameters: array<vec4f, 16>,
}

@group(2) @binding(17) var<uniform> visual_fragment_program: VisualFragmentProgram;
@group(2) @binding(19) var<storage, read> visual_properties: array<u32>;
fn visual_instruction(index: u32) -> VisualInstruction {
    return visual_fragment_program.instructions[index];
}

fn visual_parameter(index: u32) -> vec4f {
    return visual_fragment_program.parameters[index];
}

//!include "include/visual/evaluator.wgsl"
//!include "include/visual/resolve.wgsl"

fn visual_local_position(world_position: vec3f) -> vec3f {
    return atom_transform_point(model.world_to_model, world_position);
}

fn visual_world_normal(view_normal: vec3f) -> vec3f {
    return normalize(
        frame.inv_view[0].xyz * view_normal.x
            + frame.inv_view[1].xyz * view_normal.y
            + frame.inv_view[2].xyz * view_normal.z
    );
}

fn visual_material_payload(result: VisualFragmentResult) -> f32 {
    return material_payload(
        vec4f(
            result.roughness,
            result.specular,
            representation.material.z,
            result.material_strength,
        )
    );
}

fn visual_gbuffer_payload(result: VisualFragmentResult) -> f32 {
    return visual_material_payload(result) + select(0.0, 8.0, result.emission_enabled);
}

fn visual_fragment_fallback(entity_id: u32, base_color: vec4f) -> VisualFragmentResult {
    let source = atom_source_index(entity_id);
    let response = unpack4x8unorm(visual_result_word(
        source,
        VISUAL_RESULT_RESPONSE,
        pack4x8unorm(vec4f(
            visual_config.material.y,
            visual_config.material.z,
            visual_config.material.w,
            0.0,
        )),
    ));
    let geometry = unpack4x8unorm(visual_result_word(
        source,
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    ));
    return VisualFragmentResult(
        base_color,
        unpack4x8unorm(visual_result_word(
            source,
            VISUAL_RESULT_EMISSION,
            pack4x8unorm(visual_config.uniform_emission / 64.0),
        )).rgb * 64.0,
        visual_config.outputs0.z != VISUAL_MISSING,
        response.x,
        response.y,
        response.z,
        geometry.x > 0.5,
        geometry.y * 8.0,
    );
}

fn visual_fragment(
    entity_id: u32,
    base_color: vec4f,
    local_position: vec3f,
    world_position: vec3f,
    world_normal: vec3f,
) -> VisualFragmentResult {
    if !VISUAL_PROGRAM_ENABLED || visual_counts.visual_enabled == 0u {
        return VisualFragmentResult(
            base_color,
            vec3f(0.0),
            false,
            representation.material.x,
            representation.material.y,
            representation.material.w,
            true,
            0.0,
        );
    }

    let fallback = visual_fragment_fallback(entity_id, base_color);
    let camera_position = frame.inv_view[3].xyz;
    let camera_delta = camera_position - world_position;
    let camera_distance = length(camera_delta);
    let view_direction = select(
        camera_delta / camera_distance,
        vec3f(0.0),
        camera_distance <= 1.0e-8 || camera_distance != camera_distance,
    );
    let inputs = VisualEvaluationInputs(
        base_color,
        vec4f(local_position, 0.0),
        vec4f(world_position, 0.0),
        vec4f(world_normal, 0.0),
        vec4f(view_direction, 0.0),
        camera_distance,
        atom_source_index(entity_id),
    );
    return visual_resolve(inputs, fallback);
}
