// Typed visual evaluation, culling and compaction for one relation batch.

//!include "include/camera.wgsl"
//!include "include/visual/program_types.wgsl"

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct InteractionGpu {
    start_width: vec4f,
    end_period: vec4f,
    color: vec4f,
    metadata: vec4u,
    style: vec4f,
    animation: vec4f,
}

struct RelationCullConfig {
    // first row, row count, unused, unused
    counts: vec4u,
}

@group(1) @binding(0) var<storage, read> base_relations: array<InteractionGpu>;
@group(1) @binding(1) var<storage, read_write> relations: array<InteractionGpu>;
@group(1) @binding(2) var<storage, read_write> visible_relations: array<u32>;
@group(1) @binding(3) var<storage, read_write> relation_args: DrawArgs;
@group(1) @binding(4) var<uniform> relation_config: RelationCullConfig;
@group(1) @binding(5) var<storage, read> visual_instructions: array<VisualInstruction>;
@group(1) @binding(6) var<storage, read> visual_parameters: array<vec4f>;
@group(1) @binding(7) var<storage, read> visual_properties: array<u32>;
@group(1) @binding(8) var<storage, read_write> visual_results: array<u32>;
@group(1) @binding(9) var<uniform> visual_config: VisualConfig;

fn visual_instruction(index: u32) -> VisualInstruction {
    return visual_instructions[visual_config.arena_offsets.x + index];
}

fn visual_parameter(index: u32) -> vec4f {
    return visual_parameters[visual_config.arena_offsets.y + index];
}

//!include "include/visual/evaluator.wgsl"

fn relation_visual_result(
    registers: ptr<function, array<vec4f, 64>>,
    slot: u32,
    fallback: vec4f,
) -> vec4f {
    if slot == VISUAL_MISSING || visual_instruction(slot).control.w != 1u {
        return fallback;
    }
    return (*registers)[slot];
}

fn outside_plane(a: f32, aw: f32, b: f32, bw: f32) -> bool {
    return (a < -aw && b < -bw) || (a > aw && b > bw);
}

fn relation_in_frustum(relation: InteractionGpu) -> bool {
    let start = camera_world_clip(relation.start_width.xyz);
    let end = camera_world_clip(relation.end_period.xyz);
    if start.w <= 0.0 && end.w <= 0.0 {
        return false;
    }
    if outside_plane(start.x, start.w, end.x, end.w)
        || outside_plane(start.y, start.w, end.y, end.w)
        || (start.z < 0.0 && end.z < 0.0)
        || (start.z > start.w && end.z > end.w) {
        return false;
    }
    return true;
}

fn style_relation(global: u32, logical: u32) -> bool {
    let base = base_relations[global];
    var relation = relations[global];
    let midpoint = (relation.start_width.xyz + relation.end_period.xyz) * 0.5;
    let to_camera = frame.inv_view[3].xyz - midpoint;
    let distance_squared = dot(to_camera, to_camera);
    let distance = sqrt(max(distance_squared, 0.0));
    var direction = vec3f(0.0);
    if distance > 1.0e-8 {
        direction = to_camera / distance;
    }
    let inputs = VisualEvaluationInputs(
        base.color,
        vec4f(midpoint, 0.0),
        vec4f(midpoint, 0.0),
        vec4f(0.0),
        vec4f(direction, 0.0),
        distance,
        logical,
    );
    var registers: array<vec4f, 64>;
    for (var instruction_index = 0u;
        instruction_index < visual_config.counts.x;
        instruction_index++) {
        let instruction = visual_instruction(instruction_index);
        if instruction.control.w < 2u {
            registers[instruction_index] =
                visual_evaluate_instruction(instruction, inputs, &registers);
        }
    }
    let visible = visual_truth(relation_visual_result(
        &registers,
        visual_config.outputs1.z,
        vec4f(1.0),
    ));
    let width = clamp(visual_scalar_or(relation_visual_result(
        &registers,
        visual_config.outputs2.y,
        vec4f(1.0),
    ).x, 1.0), 0.0, 4.0);
    relation.start_width.w = base.start_width.w * width;
    if !visible || relation.start_width.w <= 0.0 || base.style.x <= 0.0 {
        relation.style.x = 0.0;
        relations[global] = relation;
        return false;
    }
    if !relation_in_frustum(relation) {
        return false;
    }
    let color = clamp(visual_finite_or(relation_visual_result(
        &registers,
        visual_config.outputs0.x,
        base.color,
    ), base.color), vec4f(0.0), vec4f(1.0));
    let opacity = clamp(visual_scalar_or(relation_visual_result(
        &registers,
        visual_config.outputs0.y,
        vec4f(base.style.x),
    ).x, base.style.x), 0.0, 1.0);
    let emission = clamp(visual_finite_or(relation_visual_result(
        &registers,
        visual_config.outputs0.z,
        vec4f(0.0),
    ), vec4f(0.0)), vec4f(0.0), vec4f(64.0));
    relation.color = vec4f(color.rgb + emission.rgb, color.a);
    relation.style.x = opacity;
    relations[global] = relation;
    return opacity > 0.0 && color.a > 0.0;
}

var<workgroup> visible_count: atomic<u32>;
var<workgroup> output_base: u32;

@compute @workgroup_size(64)
fn reset_relations(@builtin(global_invocation_id) id: vec3u) {
    if id.x == 0u {
        atomicStore(&relation_args.instance_count, 0u);
    }
}

@compute @workgroup_size(64)
fn cull_relations(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(local_invocation_id) local_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let logical = global_id.x + global_id.y * groups.x * 64u;
    let in_range = logical < relation_config.counts.y;
    var global = relation_config.counts.x;
    var keep = false;
    if in_range {
        global += logical;
        keep = style_relation(global, logical);
    }
    var local = 0u;
    if keep {
        local = atomicAdd(&visible_count, 1u);
    }
    let count = workgroupUniformLoad(&visible_count);
    if local_id.x == 0u && count != 0u {
        output_base = atomicAdd(&relation_args.instance_count, count);
    }
    let base = workgroupUniformLoad(&output_base);
    if keep {
        visible_relations[base + local] = global;
    }
}
