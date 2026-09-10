// Bounded typed visual-program evaluator. One invocation prepares one entity;
// fragment-only suffixes are deliberately excluded from this hot path.

//!include "include/records.wgsl"
//!include "include/visual/program_types.wgsl"

@group(0) @binding(0) var<storage, read> input_atoms: array<AtomRecord>;
// The shared cull layout owns this slot as writable because the preceding
// compaction pass fills it. This evaluator only loads from it, but WGSL access
// must match the pipeline layout exactly on native validation layers.
@group(0) @binding(2) var<storage, read_write> visible_atoms: array<u32>;
struct VisualDrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}
@group(0) @binding(4) var<storage, read_write> atom_args: VisualDrawArgs;
@group(0) @binding(8) var<uniform> visual_model_to_world: mat4x4f;
@group(0) @binding(9) var<storage, read> visual_coordinates: array<f32>;
@group(0) @binding(11) var<storage, read> visual_instructions: array<VisualInstruction>;
@group(0) @binding(12) var<storage, read> visual_parameters: array<vec4f>;
@group(0) @binding(13) var<storage, read> visual_properties: array<u32>;
@group(0) @binding(14) var<storage, read_write> visual_results: array<u32>;
@group(0) @binding(15) var<uniform> visual_config: VisualConfig;

fn visual_instruction(index: u32) -> VisualInstruction {
    return visual_instructions[visual_config.arena_offsets.x + index];
}

fn visual_parameter(index: u32) -> vec4f {
    return visual_parameters[visual_config.arena_offsets.y + index];
}

//!include "include/visual/evaluator.wgsl"

fn visual_result(registers: ptr<function, array<vec4f, 64>>, slot: u32, fallback: vec4f) -> vec4f {
    if slot == VISUAL_MISSING || visual_instruction(slot).control.w != 1u { return fallback; }
    return (*registers)[slot];
}

fn visual_bounded_offset(value: vec3f) -> vec3f {
    let maximum = visual_config.presentation.y;
    let length_squared = dot(value, value);
    if maximum <= 0.0 || !all(value == value) || any(abs(value) > vec3f(3.402823466e+38)) {
        return vec3f(0.0);
    }
    return select(value, value * maximum * inverseSqrt(length_squared), length_squared > maximum * maximum);
}

fn evaluate_visual_entity(entity: u32, allowed_results: u32) {
    if entity >= visual_config.counts.y { return; }
    let atom = input_atoms[entity];
    let source = atom.entity_id & VISUAL_ENTITY_MASK;
    let coordinate_base = source * 3u;
    let local_position = vec3f(
        visual_coordinates[coordinate_base],
        visual_coordinates[coordinate_base + 1u],
        visual_coordinates[coordinate_base + 2u],
    );
    let world_position =
        visual_model_to_world[0].xyz * local_position.x
        + visual_model_to_world[1].xyz * local_position.y
        + visual_model_to_world[2].xyz * local_position.z
        + visual_model_to_world[3].xyz;
    let inputs = VisualEvaluationInputs(
        unpack4x8unorm(atom.color),
        vec4f(local_position, 0.0),
        vec4f(world_position, 0.0),
        vec4f(0.0),
        vec4f(0.0),
        0.0,
        source,
    );
    var registers: array<vec4f, 64>;
    for (var index = 0u; index < visual_config.counts.x; index++) {
        let instruction = visual_instruction(index);
        // Fragment suffixes run only in styled fragment pipelines.
        if instruction.control.w < 2u {
            registers[index] = visual_evaluate_instruction(instruction, inputs, &registers);
        }
    }
    let base_color = inputs.base_color;
    if (visual_config.result_layout.x & VISUAL_RESULT_COLOR & allowed_results) != 0u {
        let color = clamp(visual_finite_or(visual_result(&registers, visual_config.outputs0.x, base_color), base_color), vec4f(0.0), vec4f(1.0));
        let opacity = clamp(visual_scalar_or(visual_result(&registers, visual_config.outputs0.y, vec4f(visual_config.material.x)).x, visual_config.material.x), 0.0, 1.0);
        visual_results[source] = pack4x8unorm(vec4f(color.rgb, opacity));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_EMISSION & allowed_results) != 0u {
        let emission = clamp(visual_finite_or(visual_result(&registers, visual_config.outputs0.z, vec4f(0.0)), vec4f(0.0)), vec4f(0.0), vec4f(64.0));
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_EMISSION - 1u));
        visual_results[lane * visual_config.result_layout.w + source] = pack4x8unorm(emission / 64.0);
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_RESPONSE & allowed_results) != 0u {
        let roughness = clamp(visual_scalar_or(visual_result(&registers, visual_config.outputs0.w, vec4f(visual_config.material.y)).x, visual_config.material.y), 0.05, 0.92);
        let specular = clamp(visual_scalar_or(visual_result(&registers, visual_config.outputs1.x, vec4f(visual_config.material.z)).x, visual_config.material.z), 0.0, 1.0);
        let strength = clamp(visual_scalar_or(visual_result(&registers, visual_config.outputs1.y, vec4f(visual_config.material.w)).x, visual_config.material.w), 0.0, 1.0);
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_RESPONSE - 1u));
        visual_results[lane * visual_config.result_layout.w + source] = pack4x8unorm(vec4f(roughness, specular, strength, 0.0));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_GEOMETRY & allowed_results) != 0u {
        let visible = visual_truth(visual_result(&registers, visual_config.outputs1.z, vec4f(1.0)));
        let softness = clamp(visual_scalar_or(visual_result(&registers, visual_config.outputs1.w, vec4f(0.0)).x, 0.0) / 8.0, 0.0, 1.0);
        let radius = clamp(visual_scalar_or(visual_result(&registers, visual_config.outputs2.x, vec4f(1.0)).x, 1.0) / 4.0, 0.0, 1.0);
        let width = clamp(visual_scalar_or(visual_result(&registers, visual_config.outputs2.y, vec4f(1.0)).x, 1.0) / 4.0, 0.0, 1.0);
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_GEOMETRY - 1u));
        visual_results[lane * visual_config.result_layout.w + source] = pack4x8unorm(vec4f(select(0.0, 1.0, visible), softness, radius, width));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_OFFSET & allowed_results) != 0u {
        let offset = visual_bounded_offset(visual_result(&registers, visual_config.outputs2.z, vec4f(0.0)).xyz);
        let base = visual_config.result_layout.z + source * 3u;
        visual_results[base] = bitcast<u32>(offset.x);
        visual_results[base + 1u] = bitcast<u32>(offset.y);
        visual_results[base + 2u] = bitcast<u32>(offset.z);
    }
}

@compute @workgroup_size(64)
fn evaluate_visual_cull(@builtin(global_invocation_id) id: vec3u) {
    evaluate_visual_entity(id.x, VISUAL_RESULT_GEOMETRY | VISUAL_RESULT_OFFSET);
}

@compute @workgroup_size(64)
fn evaluate_visual_shading(@builtin(global_invocation_id) id: vec3u) {
    let compact = id.x;
    if compact >= atomicLoad(&atom_args.instance_count) { return; }
    evaluate_visual_entity(
        visible_atoms[compact],
        VISUAL_RESULT_COLOR | VISUAL_RESULT_EMISSION | VISUAL_RESULT_RESPONSE,
    );
}

@compute @workgroup_size(64)
fn evaluate_visual_shading_all(@builtin(global_invocation_id) id: vec3u) {
    evaluate_visual_entity(
        id.x,
        VISUAL_RESULT_COLOR | VISUAL_RESULT_EMISSION | VISUAL_RESULT_RESPONSE,
    );
}
