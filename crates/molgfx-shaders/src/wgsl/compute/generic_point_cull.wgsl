// Branch-free culling and indirect argument generation for generic points.

//!include "include/camera.wgsl"
//!include "include/visual/program_types.wgsl"

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct PointCullOutput {
    args: DrawArgs,
    visible: array<u32>,
}

struct GenericPointConfig {
    source: vec4u,
    picking: vec4u,
    timeline: vec4f,
}

@group(1) @binding(0) var<storage, read> point_positions: array<u32>;
@group(1) @binding(1) var<storage, read_write> point_output: PointCullOutput;
@group(1) @binding(3) var<uniform> point_config: GenericPointConfig;
@group(1) @binding(4) var<storage, read_write> point_tiles: array<atomic<u32>>;
@group(1) @binding(5) var<storage, read> visual_instructions: array<VisualInstruction>;
@group(1) @binding(6) var<storage, read> visual_parameters: array<vec4f>;
@group(1) @binding(7) var<storage, read> visual_properties: array<u32>;
@group(1) @binding(8) var<storage, read_write> visual_results: array<u32>;
@group(1) @binding(9) var<uniform> visual_config: VisualConfig;
@group(1) @binding(10) var<storage, read> point_positions_end: array<u32>;

fn visual_instruction(index: u32) -> VisualInstruction {
    return visual_instructions[visual_config.arena_offsets.x + index];
}

fn visual_parameter(index: u32) -> vec4f {
    return visual_parameters[visual_config.arena_offsets.y + index];
}

//!include "include/visual/evaluator.wgsl"

fn generic_point_visual_result(
    registers: ptr<function, array<vec4f, 64>>,
    slot: u32,
    fallback: vec4f,
) -> vec4f {
    if slot == VISUAL_MISSING || visual_instruction(slot).control.w != 1u {
        return fallback;
    }
    return (*registers)[slot];
}

fn generic_point_bounded_offset(value: vec3f) -> vec3f {
    let maximum = visual_config.presentation.y;
    let length_squared = dot(value, value);
    if maximum <= 0.0 || !all(value == value) || any(abs(value) > vec3f(3.402823466e+38)) {
        return vec3f(0.0);
    }
    return select(
        value,
        value * maximum * inverseSqrt(length_squared),
        length_squared > maximum * maximum,
    );
}

fn evaluate_generic_point_visual(index: u32, allowed_results: u32) {
    if index >= visual_config.counts.y || visual_config.counts.x == 0u {
        return;
    }
    let position = point_position(index);
    let inputs = VisualEvaluationInputs(
        unpack4x8unorm(point_config.source.z),
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
        let color = clamp(
            visual_finite_or(
                generic_point_visual_result(&registers, visual_config.outputs0.x, inputs.base_color),
                inputs.base_color,
            ),
            vec4f(0.0),
            vec4f(1.0),
        );
        let opacity = clamp(
            visual_scalar_or(
                generic_point_visual_result(
                    &registers,
                    visual_config.outputs0.y,
                    vec4f(visual_config.material.x),
                ).x,
                visual_config.material.x,
            ),
            0.0,
            1.0,
        );
        visual_results[index] = pack4x8unorm(vec4f(color.rgb, opacity));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_EMISSION & allowed_results) != 0u {
        let emission = clamp(
            visual_finite_or(
                generic_point_visual_result(&registers, visual_config.outputs0.z, vec4f(0.0)),
                vec4f(0.0),
            ),
            vec4f(0.0),
            vec4f(64.0),
        );
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_EMISSION - 1u));
        visual_results[lane * visual_config.result_layout.w + index] = pack4x8unorm(emission / 64.0);
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_RESPONSE & allowed_results) != 0u {
        let roughness = clamp(visual_scalar_or(generic_point_visual_result(&registers, visual_config.outputs0.w, vec4f(visual_config.material.y)).x, visual_config.material.y), 0.05, 0.92);
        let specular = clamp(visual_scalar_or(generic_point_visual_result(&registers, visual_config.outputs1.x, vec4f(visual_config.material.z)).x, visual_config.material.z), 0.0, 1.0);
        let strength = clamp(visual_scalar_or(generic_point_visual_result(&registers, visual_config.outputs1.y, vec4f(visual_config.material.w)).x, visual_config.material.w), 0.0, 1.0);
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_RESPONSE - 1u));
        visual_results[lane * visual_config.result_layout.w + index] = pack4x8unorm(vec4f(roughness, specular, strength, 0.0));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_GEOMETRY & allowed_results) != 0u {
        let visible = visual_truth(generic_point_visual_result(&registers, visual_config.outputs1.z, vec4f(1.0)));
        let softness = clamp(visual_scalar_or(generic_point_visual_result(&registers, visual_config.outputs1.w, vec4f(0.0)).x, 0.0) / 8.0, 0.0, 1.0);
        let radius = clamp(visual_scalar_or(generic_point_visual_result(&registers, visual_config.outputs2.x, vec4f(1.0)).x, 1.0) / 4.0, 0.0, 1.0);
        let width = clamp(visual_scalar_or(generic_point_visual_result(&registers, visual_config.outputs2.y, vec4f(1.0)).x, 1.0) / 4.0, 0.0, 1.0);
        let lane = countOneBits(visual_config.result_layout.x & (VISUAL_RESULT_GEOMETRY - 1u));
        visual_results[lane * visual_config.result_layout.w + index] = pack4x8unorm(vec4f(select(0.0, 1.0, visible), softness, radius, width));
    }
    if (visual_config.result_layout.x & VISUAL_RESULT_OFFSET & allowed_results) != 0u {
        let offset = generic_point_bounded_offset(
            generic_point_visual_result(&registers, visual_config.outputs2.z, vec4f(0.0)).xyz,
        );
        let base = visual_config.result_layout.z + index * 3u;
        visual_results[base] = bitcast<u32>(offset.x);
        visual_results[base + 1u] = bitcast<u32>(offset.y);
        visual_results[base + 2u] = bitcast<u32>(offset.z);
    }
}

fn generic_point_geometry(index: u32) -> vec4f {
    return unpack4x8unorm(visual_result_word(
        index,
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    ));
}

var<workgroup> visible_count: atomic<u32>;
var<workgroup> output_base: u32;

const POINT_TILE_SIZE: u32 = 8u;
const POINT_TILE_INDEX_MASK: u32 = 0x00ffffffu;
const POINT_TILE_MAX_INDEX: u32 = POINT_TILE_INDEX_MASK - 1u;
const POINT_LOD_RADIUS_PIXELS: f32 = 8.0;

struct PointProjection {
    visible: bool,
    tile: u32,
    packed: u32,
    lod: bool,
}

fn point_position(index: u32) -> vec3f {
    let base = index * 3u;
    let start = vec3f(
        bitcast<f32>(point_positions[base]),
        bitcast<f32>(point_positions[base + 1u]),
        bitcast<f32>(point_positions[base + 2u]),
    );
    let end = vec3f(
        bitcast<f32>(point_positions_end[base]),
        bitcast<f32>(point_positions_end[base + 1u]),
        bitcast<f32>(point_positions_end[base + 2u]),
    );
    return mix(start, end, point_config.timeline.x);
}

fn project_point(index: u32) -> PointProjection {
    if index >= point_config.source.x || ((point_config.source.z >> 24u) & 0xffu) == 0u {
        return PointProjection(false, 0u, 0u, false);
    }
    let geometry = generic_point_geometry(index);
    if geometry.x <= 0.5 {
        return PointProjection(false, 0u, 0u, false);
    }
    let radius = bitcast<f32>(point_config.source.y) * geometry.z * 4.0;
    let clip = camera_world_clip(point_position(index) + visual_result_offset(index));
    if clip.w <= 0.0 || clip.z < 0.0 || clip.z > clip.w {
        return PointProjection(false, 0u, 0u, false);
    }
    let extent = frame.projection_kind.yz * radius;
    if !all(abs(clip.xy) <= vec2f(clip.w) + extent) {
        return PointProjection(false, 0u, 0u, false);
    }
    let ndc = clip.xy / clip.w;
    let pixel = clamp(
        (ndc * vec2f(0.5, -0.5) + vec2f(0.5)) * frame.viewport.xy,
        vec2f(0.0),
        max(frame.viewport.xy - vec2f(1.0), vec2f(0.0)),
    );
    let tile_xy = vec2u(pixel) / vec2u(POINT_TILE_SIZE);
    let tile = tile_xy.y * point_config.picking.y + tile_xy.x;
    let radius_pixels = max(
        extent.x / clip.w * frame.viewport.x * 0.5,
        extent.y / clip.w * frame.viewport.y * 0.5,
    );
    let lod = point_config.picking.w != 0u && radius_pixels <= POINT_LOD_RADIUS_PIXELS;
    let depth = u32(round(clamp(clip.z / clip.w, 0.0, 1.0) * 255.0));
    let tie = POINT_TILE_INDEX_MASK - min(index, POINT_TILE_MAX_INDEX);
    return PointProjection(true, tile, (depth << 24u) | tie, lod);
}

@compute @workgroup_size(64)
fn evaluate_generic_point_visual_cull(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = global_id.x + global_id.y * groups.x * 64u;
    evaluate_generic_point_visual(index, VISUAL_RESULT_GEOMETRY | VISUAL_RESULT_OFFSET);
}

@compute @workgroup_size(64)
fn evaluate_generic_point_visual_shading(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let compact = global_id.x + global_id.y * groups.x * 64u;
    if compact >= atomicLoad(&point_output.args.instance_count) {
        return;
    }
    evaluate_generic_point_visual(
        point_output.visible[compact],
        VISUAL_RESULT_COLOR | VISUAL_RESULT_EMISSION | VISUAL_RESULT_RESPONSE,
    );
}

@compute @workgroup_size(64)
fn reset_generic_points(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = id.x + id.y * groups.x * 64u;
    if index == 0u {
        atomicStore(&point_output.args.instance_count, 0u);
    }
    if index < point_config.picking.z {
        atomicStore(&point_tiles[index], 0u);
    }
}

@compute @workgroup_size(64)
fn bin_generic_points(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(local_invocation_id) local_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = global_id.x + global_id.y * groups.x * 64u;
    let projection = project_point(index);
    if projection.visible && projection.lod {
        atomicMax(&point_tiles[projection.tile], projection.packed);
    }
    let keep = projection.visible && !projection.lod;
    var local = 0u;
    if keep {
        local = atomicAdd(&visible_count, 1u);
    }
    let count = workgroupUniformLoad(&visible_count);
    if local_id.x == 0u && count != 0u {
        output_base = atomicAdd(&point_output.args.instance_count, count);
    }
    let base = workgroupUniformLoad(&output_base);
    if keep {
        point_output.visible[base + local] = index;
    }
}

@compute @workgroup_size(64)
fn compact_generic_point_tiles(
    @builtin(global_invocation_id) global_id: vec3u,
    @builtin(local_invocation_id) local_id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let tile = global_id.x + global_id.y * groups.x * 64u;
    var packed = 0u;
    if tile < point_config.picking.z {
        packed = atomicLoad(&point_tiles[tile]);
    }
    let keep = packed != 0u;
    var local = 0u;
    if keep {
        local = atomicAdd(&visible_count, 1u);
    }
    let count = workgroupUniformLoad(&visible_count);
    if local_id.x == 0u && count != 0u {
        output_base = atomicAdd(&point_output.args.instance_count, count);
    }
    let base = workgroupUniformLoad(&output_base);
    if keep {
        point_output.visible[base + local] = POINT_TILE_INDEX_MASK - (packed & POINT_TILE_INDEX_MASK);
    }
}
