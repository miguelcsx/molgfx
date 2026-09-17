// Branch-free rigid-instance culling and two indirect analytic draw commands.

//!include "include/camera.wgsl"
//!include "include/visual/program_types.wgsl"

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct InstanceCullOutput {
    args: array<DrawArgs, 2>,
    visible: array<u32>,
}

struct RigidInstanceGpu {
    translation_scale: vec4f,
    orientation: vec4f,
}

struct GenericInstanceConfig {
    counts: vec4u,
    picking_style: vec4u,
    bounds: vec4f,
}

@group(1) @binding(0) var<storage, read> instance_transforms: array<RigidInstanceGpu>;
@group(1) @binding(1) var<storage, read_write> instance_output: InstanceCullOutput;
@group(1) @binding(3) var<uniform> instance_config: GenericInstanceConfig;
@group(1) @binding(4) var<storage, read> visual_instructions: array<VisualInstruction>;
@group(1) @binding(5) var<storage, read> visual_parameters: array<vec4f>;
@group(1) @binding(6) var<storage, read> visual_properties: array<u32>;
@group(1) @binding(7) var<storage, read_write> visual_results: array<u32>;
@group(1) @binding(8) var<uniform> visual_config: VisualConfig;
@group(1) @binding(9) var<storage, read> instance_frame_start: array<RigidInstanceGpu>;
@group(1) @binding(10) var<storage, read> instance_frame_end: array<RigidInstanceGpu>;

fn sampled_instance(index: u32) -> RigidInstanceGpu {
    if instance_config.bounds.z < 0.5 {
        return instance_transforms[instance_config.picking_style.z + index];
    }
    let start = instance_frame_start[instance_config.picking_style.w + index];
    let end = instance_frame_end[bitcast<u32>(instance_config.bounds.w) + index];
    var end_orientation = end.orientation;
    if dot(start.orientation, end_orientation) < 0.0 {
        end_orientation = -end_orientation;
    }
    return RigidInstanceGpu(
        mix(start.translation_scale, end.translation_scale, instance_config.bounds.y),
        normalize(mix(start.orientation, end_orientation, instance_config.bounds.y)),
    );
}

fn visual_instruction(index: u32) -> VisualInstruction {
    return visual_instructions[visual_config.arena_offsets.x + index];
}

fn visual_parameter(index: u32) -> vec4f {
    return visual_parameters[visual_config.arena_offsets.y + index];
}

//!include "include/visual/evaluator.wgsl"

fn generic_instance_visual_result(
    registers: ptr<function, array<vec4f, 64>>,
    slot: u32,
    fallback: vec4f,
) -> vec4f {
    if slot == VISUAL_MISSING || visual_instruction(slot).control.w != 1u {
        return fallback;
    }
    return (*registers)[slot];
}

fn generic_instance_bounded_offset(value: vec3f) -> vec3f {
    let maximum = visual_config.presentation.y;
    let length_squared = dot(value, value);
    if maximum <= 0.0 || !all(value == value) || any(abs(value) > vec3f(3.402823466e+38)) {
        return vec3f(0.0);
    }
    return select(value, value * maximum * inverseSqrt(length_squared), length_squared > maximum * maximum);
}

fn evaluate_generic_instance_visual(index: u32, allowed_results: u32) {
    if index >= visual_config.counts.y || visual_config.counts.x == 0u {
        return;
    }
    let position = sampled_instance(index).translation_scale.xyz;
    let inputs = VisualEvaluationInputs(
        unpack4x8unorm(instance_config.picking_style.y),
        vec4f(position, 0.0),
        vec4f(position, 0.0),
        vec4f(0.0),
        vec4f(0.0),
        0.0,
        index,
    );
    var registers: array<vec4f, 64>;
    for (var instruction_index = 0u; instruction_index < visual_config.counts.x; instruction_index++) {
        let instruction = visual_instruction(instruction_index);
        if instruction.control.w < 2u {
            registers[instruction_index] = visual_evaluate_instruction(instruction, inputs, &registers);
        }
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_COLOR & allowed_results) != 0u {
        let color = clamp(visual_finite_or(generic_instance_visual_result(&registers, visual_config.outputs0.x, inputs.base_color), inputs.base_color), vec4f(0.0), vec4f(1.0));
        let opacity = clamp(visual_scalar_or(generic_instance_visual_result(&registers, visual_config.outputs0.y, vec4f(visual_config.material.x)).x, visual_config.material.x), 0.0, 1.0);
        visual_results[index] = pack4x8unorm(vec4f(color.rgb, opacity));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_EMISSION & allowed_results) != 0u {
        let emission = clamp(visual_finite_or(generic_instance_visual_result(&registers, visual_config.outputs0.z, vec4f(0.0)), vec4f(0.0)), vec4f(0.0), vec4f(64.0));
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_EMISSION - 1u));
        visual_results[lane * visual_config.result_layout.w + index] = pack4x8unorm(emission / 64.0);
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_RESPONSE & allowed_results) != 0u {
        let roughness = clamp(visual_scalar_or(generic_instance_visual_result(&registers, visual_config.outputs0.w, vec4f(visual_config.material.y)).x, visual_config.material.y), 0.05, 0.92);
        let specular = clamp(visual_scalar_or(generic_instance_visual_result(&registers, visual_config.outputs1.x, vec4f(visual_config.material.z)).x, visual_config.material.z), 0.0, 1.0);
        let strength = clamp(visual_scalar_or(generic_instance_visual_result(&registers, visual_config.outputs1.y, vec4f(visual_config.material.w)).x, visual_config.material.w), 0.0, 1.0);
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_RESPONSE - 1u));
        visual_results[lane * visual_config.result_layout.w + index] = pack4x8unorm(vec4f(roughness, specular, strength, 0.0));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_GEOMETRY & allowed_results) != 0u {
        let visible = visual_truth(generic_instance_visual_result(&registers, visual_config.outputs1.z, vec4f(1.0)));
        let softness = clamp(visual_scalar_or(generic_instance_visual_result(&registers, visual_config.outputs1.w, vec4f(0.0)).x, 0.0) / 8.0, 0.0, 1.0);
        let radius = clamp(visual_scalar_or(generic_instance_visual_result(&registers, visual_config.outputs2.x, vec4f(1.0)).x, 1.0) / 4.0, 0.0, 1.0);
        let width = clamp(visual_scalar_or(generic_instance_visual_result(&registers, visual_config.outputs2.y, vec4f(1.0)).x, 1.0) / 4.0, 0.0, 1.0);
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_GEOMETRY - 1u));
        visual_results[lane * visual_config.result_layout.w + index] = pack4x8unorm(vec4f(select(0.0, 1.0, visible), softness, radius, width));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_OFFSET & allowed_results) != 0u {
        let offset = generic_instance_bounded_offset(generic_instance_visual_result(&registers, visual_config.outputs2.z, vec4f(0.0)).xyz);
        let base = visual_config.result_layout.z + index * 3u;
        visual_results[base] = bitcast<u32>(offset.x);
        visual_results[base + 1u] = bitcast<u32>(offset.y);
        visual_results[base + 2u] = bitcast<u32>(offset.z);
    }
}

fn generic_instance_geometry(index: u32) -> vec4f {
    return unpack4x8unorm(visual_result_word(index, VISUAL_RESULT_GEOMETRY, pack4x8unorm(visual_config.uniform_geometry)));
}

var<workgroup> visible_count: atomic<u32>;
var<workgroup> output_base: u32;

fn instance_visible(index: u32) -> bool {
    if index >= instance_config.counts.x {
        return false;
    }
    let transform = sampled_instance(index);
    let geometry = generic_instance_geometry(index);
    if geometry.x <= 0.5 {
        return false;
    }
    let radius = instance_config.bounds.x * transform.translation_scale.w * geometry.z * 4.0;
    let clip = camera_world_clip(transform.translation_scale.xyz + visual_result_offset(index));
    if clip.w <= 0.0 || clip.z < 0.0 || clip.z > clip.w {
        return false;
    }
    let extent = frame.projection_kind.yz * radius;
    return all(abs(clip.xy) <= vec2f(clip.w) + extent);
}

@compute @workgroup_size(64)
fn evaluate_generic_instance_visual_cull(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = global_id.x + global_id.y * groups.x * 64u;
    evaluate_generic_instance_visual(index, VISUAL_RESULT_GEOMETRY | VISUAL_RESULT_OFFSET);
}

@compute @workgroup_size(64)
fn evaluate_generic_instance_visual_shading(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let compact = global_id.x + global_id.y * groups.x * 64u;
    let visible_count = select(
        atomicLoad(&instance_output.args[1].instance_count) / max(instance_config.counts.z, 1u),
        atomicLoad(&instance_output.args[0].instance_count) / max(instance_config.counts.y, 1u),
        instance_config.counts.y != 0u,
    );
    if compact >= visible_count {
        return;
    }
    evaluate_generic_instance_visual(
        instance_output.visible[compact],
        VISUAL_RESULT_COLOR | VISUAL_RESULT_EMISSION | VISUAL_RESULT_RESPONSE,
    );
}

@compute @workgroup_size(64)
fn reset_generic_instances(@builtin(global_invocation_id) id: vec3u) {
    if id.x == 0u {
        atomicStore(&instance_output.args[0].instance_count, 0u);
        atomicStore(&instance_output.args[1].instance_count, 0u);
    }
}

@compute @workgroup_size(64)
fn cull_generic_instances(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(local_invocation_id) local_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = global_id.x + global_id.y * groups.x * 64u;
    let keep = instance_visible(index);
    var local = 0u;
    if keep {
        local = atomicAdd(&visible_count, 1u);
    }
    let count = workgroupUniformLoad(&visible_count);
    if local_id.x == 0u && count != 0u {
        if instance_config.counts.y != 0u {
            let expanded = atomicAdd(
                &instance_output.args[0].instance_count,
                count * instance_config.counts.y,
            );
            output_base = expanded / instance_config.counts.y;
            if instance_config.counts.z != 0u {
                atomicAdd(
                    &instance_output.args[1].instance_count,
                    count * instance_config.counts.z,
                );
            }
        } else {
            let expanded = atomicAdd(
                &instance_output.args[1].instance_count,
                count * instance_config.counts.z,
            );
            output_base = expanded / instance_config.counts.z;
        }
    }
    let base = workgroupUniformLoad(&output_base);
    if keep {
        instance_output.visible[base + local] = index;
    }
}
