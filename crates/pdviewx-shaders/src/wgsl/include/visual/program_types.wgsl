// Fixed visual-program records shared by compute and fragment evaluators.

struct VisualInstruction {
    // opcode, value kind, packed operands, earliest stage
    control: vec4u,
    data: vec4f,
}

struct VisualConfig {
    // instruction count, entity count, fragment instruction count, parameter count
    counts: vec4u,
    // color, opacity, emission, roughness result registers
    outputs0: vec4u,
    // specular, model strength, visibility, softness result registers
    outputs1: vec4u,
    // radius, width, offset and unused result registers
    outputs2: vec4u,
    base_color: vec4f,
    // opacity, roughness, specular, model-specific strength
    material: vec4f,
    uniform_emission: vec4f,
    // visibility, softness / 8, radius / 4, width / 4
    uniform_geometry: vec4f,
    uniform_offset: vec4f,
    // time seconds, maximum displacement, unused, unused
    presentation: vec4f,
    // Scene-wide typed-column word offsets; zero addresses the missing NaN value.
    property_offsets: vec4u,
    // Low byte is stride in words; high byte is AttributeKind.
    attribute_layouts: vec4u,
    // Optional second page and interpolation for directly sampled paged timelines.
    property_end_offsets: vec4u,
    property_alphas: vec4f,
    // Instruction and parameter base offsets in persistent scene arenas.
    arena_offsets: vec4u,
    // Entity-lane mask, uniform-output mask, optional XYZ base, row stride.
    result_layout: vec4u,
}

struct VisualEvaluationInputs {
    base_color: vec4f,
    local_position: vec4f,
    world_position: vec4f,
    normal: vec4f,
    view_direction: vec4f,
    camera_distance: f32,
    entity: u32,
}

const VISUAL_MISSING: u32 = 0xffffffffu;
const VISUAL_ENTITY_MASK: u32 = 0x1fffffffu;
const VISUAL_RESULT_COLOR: u32 = 1u;
const VISUAL_RESULT_EMISSION: u32 = 2u;
const VISUAL_RESULT_RESPONSE: u32 = 4u;
const VISUAL_RESULT_GEOMETRY: u32 = 8u;
const VISUAL_RESULT_OFFSET: u32 = 16u;
const VISUAL_OUTPUT_BASE_COLOR: u32 = 0u;
const VISUAL_OUTPUT_OPACITY: u32 = 1u;
const VISUAL_OUTPUT_EMISSION: u32 = 2u;
const VISUAL_OUTPUT_ROUGHNESS: u32 = 3u;
const VISUAL_OUTPUT_SPECULAR: u32 = 4u;
const VISUAL_OUTPUT_MATERIAL_STRENGTH: u32 = 5u;
const VISUAL_OUTPUT_VISIBILITY: u32 = 6u;
const VISUAL_OUTPUT_SOFTNESS: u32 = 7u;
const VISUAL_OUTPUT_RADIUS: u32 = 8u;
const VISUAL_OUTPUT_WIDTH: u32 = 9u;
const VISUAL_OUTPUT_POSITION_OFFSET: u32 = 10u;

fn visual_output_is_uniform(output: u32) -> bool {
    return (visual_config.result_layout.y & (1u << output)) != 0u;
}

fn visual_uniform_base_color(value: vec4f) -> vec4f {
    var result = value;
    if visual_output_is_uniform(VISUAL_OUTPUT_BASE_COLOR) {
        result = vec4f(visual_config.base_color.rgb, result.a);
    }
    if visual_output_is_uniform(VISUAL_OUTPUT_OPACITY) {
        result.a = visual_config.base_color.a;
    }
    return result;
}

fn visual_result_word(source: u32, lane: u32, fallback: u32) -> u32 {
    if (visual_config.result_layout.x & lane) == 0u {
        return fallback;
    }
    let preceding = countOneBits(visual_config.result_layout.x & (lane - 1u));
    let index = preceding * visual_config.result_layout.w
        + source;
    return visual_results[index];
}

fn visual_result_offset(source: u32) -> vec3f {
    if (visual_config.result_layout.x & VISUAL_RESULT_OFFSET) == 0u {
        return select(
            vec3f(0.0),
            visual_config.uniform_offset.xyz,
            visual_output_is_uniform(VISUAL_OUTPUT_POSITION_OFFSET),
        );
    }
    let base = visual_config.result_layout.z + source * 3u;
    return vec3f(
        bitcast<f32>(visual_results[base]),
        bitcast<f32>(visual_results[base + 1u]),
        bitcast<f32>(visual_results[base + 2u]),
    );
}
