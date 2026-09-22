// Typed visual programs for indexed caller/ribbon geometry.

override VISUAL_PROGRAM_ENABLED: bool = false;
override VISUAL_FRAGMENT_ENABLED: bool = false;

//!include "include/visual/program_types.wgsl"

@group(2) @binding(8) var<storage, read> visual_results: array<u32>;
@group(2) @binding(9) var<storage, read> visual_instructions: array<VisualInstruction>;
@group(2) @binding(10) var<storage, read> visual_parameters: array<vec4f>;
@group(2) @binding(11) var<storage, read> visual_properties: array<u32>;
@group(2) @binding(12) var<uniform> visual_config: VisualConfig;

fn visual_instruction(index: u32) -> VisualInstruction {
    return visual_instructions[visual_config.arena_offsets.x + index];
}

fn visual_parameter(index: u32) -> vec4f {
    return visual_parameters[visual_config.arena_offsets.y + index];
}

//!include "include/visual/evaluator.wgsl"
//!include "include/visual/resolve.wgsl"

fn ribbon_visual_source(entity_id: u32) -> u32 {
    return entity_id & VISUAL_ENTITY_MASK;
}

fn ribbon_visual_offset(entity_id: u32) -> vec3f {
    if !VISUAL_PROGRAM_ENABLED {
        return vec3f(0.0);
    }
    return visual_result_offset(ribbon_visual_source(entity_id));
}

fn ribbon_visual_fallback(entity_id: u32, base_color: vec4f) -> VisualFragmentResult {
    let source = ribbon_visual_source(entity_id);
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
        visual_uniform_base_color(base_color),
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

fn ribbon_visual(
    entity_id: u32,
    base_color: vec4f,
    local_position: vec3f,
    world_position: vec3f,
    world_normal: vec3f,
) -> VisualFragmentResult {
    var presented_color = base_color;
    presented_color.a *= ribbon_uniforms.presentation.x;
    if !VISUAL_PROGRAM_ENABLED {
        return VisualFragmentResult(
            presented_color,
            vec3f(0.0),
            false,
            ribbon_uniforms.material.x,
            ribbon_uniforms.material.y,
            ribbon_uniforms.material.w,
            true,
            0.0,
        );
    }
    let camera_delta = frame.inv_view[3].xyz - world_position;
    let camera_distance = length(camera_delta);
    let view_direction = select(
        camera_delta / camera_distance,
        vec3f(0.0),
        camera_distance <= 1.0e-8 || camera_distance != camera_distance,
    );
    return visual_resolve(
        VisualEvaluationInputs(
            presented_color,
            vec4f(local_position, 0.0),
            vec4f(world_position, 0.0),
            vec4f(world_normal, 0.0),
            vec4f(view_direction, 0.0),
            camera_distance,
            ribbon_visual_source(entity_id),
        ),
        ribbon_visual_fallback(entity_id, presented_color),
    );
}

fn ribbon_visual_material(result: VisualFragmentResult) -> f32 {
    return ribbon_material_payload(
        vec4f(
            result.roughness,
            result.specular,
            ribbon_uniforms.material.z,
            result.material_strength,
        )
    );
}

fn ribbon_visual_gbuffer_material(result: VisualFragmentResult) -> f32 {
    return ribbon_visual_material(result) + select(0.0, 8.0, result.emission_enabled);
}
