// Shared bounded interpreter used by entity compute and styled fragments.
// Each consumer supplies visual_instruction and visual_parameter accessors so
// compute can use scene arenas while fragments use one portable uniform block.

fn visual_nan(payload: f32) -> f32 {
    // Keep the conversion runtime-dependent. WebGPU's WGSL validator rejects
    // a constant expression whose result is NaN, even though a NaN produced
    // while evaluating shader data is valid.
    return bitcast<f32>(bitcast<u32>(payload) | 0x7fc00000u);
}

fn visual_operand(packed: u32, index: u32) -> u32 {
    return (packed >> (index * 8u)) & 0xffu;
}

fn visual_truth(value: vec4f) -> bool {
    return value.x != 0.0
        && value.x == value.x
        && abs(value.x) <= 3.402823466e+38;
}

fn visual_ordered_minimum(left: f32, right: f32) -> f32 {
    return select(min(left, right), visual_nan(left), left != left || right != right);
}

fn visual_ordered_maximum(left: f32, right: f32) -> f32 {
    return select(max(left, right), visual_nan(left), left != left || right != right);
}

fn visual_ordered_clamp(value: f32, low: f32, high: f32) -> f32 {
    let valid = value == value
        && low == low
        && high == high
        && abs(value) <= 3.402823466e+38
        && abs(low) <= 3.402823466e+38
        && abs(high) <= 3.402823466e+38
        && low <= high;
    return select(visual_nan(value), min(max(value, low), high), valid);
}

fn visual_ordered_step(edge: f32, value: f32) -> f32 {
    let valid = edge == edge
        && value == value
        && abs(edge) <= 3.402823466e+38
        && abs(value) <= 3.402823466e+38;
    return select(0.0, 1.0, valid && value >= edge);
}

fn visual_input(slot: u32, inputs: VisualEvaluationInputs) -> vec4f {
    switch slot {
        case 0u: { return inputs.base_color; }
        case 1u: { return vec4f(visual_config.material.x, 0.0, 0.0, 0.0); }
        case 2u: { return vec4f(visual_config.presentation.x, 0.0, 0.0, 0.0); }
        case 3u: { return inputs.local_position; }
        case 4u: { return inputs.world_position; }
        case 5u: { return inputs.normal; }
        case 6u: { return inputs.view_direction; }
        case 7u: { return vec4f(inputs.camera_distance, 0.0, 0.0, 0.0); }
        case 8u: { return vec4f(f32(inputs.entity), 0.0, 0.0, 0.0); }
        case 9u: { return vec4f(visual_config.material.y, 0.0, 0.0, 0.0); }
        case 10u: { return vec4f(visual_config.material.z, 0.0, 0.0, 0.0); }
        case 11u: { return vec4f(visual_config.material.w, 0.0, 0.0, 0.0); }
        default: { return vec4f(0.0); }
    }
}

fn visual_component_count(kind: u32) -> u32 {
    return select(select(1u, 3u, kind == 3u), 4u, kind == 1u);
}

fn visual_componentwise(kind: u32, left: vec4f, right: vec4f, opcode: u32) -> vec4f {
    var result = vec4f(0.0);
    let count = visual_component_count(kind);
    for (var component = 0u; component < count; component++) {
        if opcode == 4u { result[component] = left[component] + right[component]; }
        if opcode == 5u { result[component] = left[component] - right[component]; }
        if opcode == 6u { result[component] = left[component] * right[component]; }
    }
    return result;
}

fn visual_evaluate_instruction(
    instruction: VisualInstruction,
    inputs: VisualEvaluationInputs,
    registers: ptr<function, array<vec4f, 64>>,
) -> vec4f {
    let opcode = instruction.control.x;
    let kind = instruction.control.y;
    let packed = instruction.control.z;
    let left = (*registers)[visual_operand(packed, 0u)];
    let right = (*registers)[visual_operand(packed, 1u)];
    let third = (*registers)[visual_operand(packed, 2u)];
    if opcode == 0u { return instruction.data; }
    if opcode == 1u { return visual_input(u32(instruction.data.x), inputs); }
    if opcode == 2u {
        let property = u32(instruction.data.x);
        let attribute_layout = visual_config.attribute_layouts[property];
        let stride = max(attribute_layout & 0xffu, 1u);
        let physical = (attribute_layout >> 8u) & 0xffu;
        let base = visual_config.property_offsets[property] + inputs.entity * stride;
        let temporal = ((attribute_layout >> 16u) & 1u) != 0u;
        let paged_temporal = ((attribute_layout >> 17u) & 1u) != 0u;
        var sampled_base = base;
        var sampled_alpha = 0.0;
        var sampled_end = base;
        if temporal {
            sampled_alpha = bitcast<f32>(visual_properties[visual_config.property_offsets[property] - 1u]);
            sampled_end = visual_properties[visual_config.property_offsets[property] - 2u]
                + inputs.entity * stride;
        } else if paged_temporal {
            sampled_alpha = visual_config.property_alphas[property];
            sampled_end = visual_config.property_end_offsets[property]
                + inputs.entity * stride;
        }
        if physical == 1u {
            return vec4f(f32(visual_properties[sampled_base]), 0.0, 0.0, 0.0);
        }
        if physical == 2u {
            let start_value = vec3f(
                bitcast<f32>(visual_properties[sampled_base]),
                bitcast<f32>(visual_properties[sampled_base + 1u]),
                bitcast<f32>(visual_properties[sampled_base + 2u]),
            );
            let end_value = vec3f(
                bitcast<f32>(visual_properties[sampled_end]),
                bitcast<f32>(visual_properties[sampled_end + 1u]),
                bitcast<f32>(visual_properties[sampled_end + 2u]),
            );
            return vec4f(
                mix(start_value, end_value, sampled_alpha),
                0.0
            );
        }
        if physical == 3u {
            return unpack4x8unorm(visual_properties[base]);
        }
        let start_value = bitcast<f32>(visual_properties[sampled_base]);
        let end_value = bitcast<f32>(visual_properties[sampled_end]);
        return vec4f(mix(start_value, end_value, sampled_alpha), 0.0, 0.0, 0.0);
    }
    if opcode == 3u { return visual_parameter(u32(instruction.data.x)); }
    if opcode >= 4u && opcode <= 6u {
        if opcode == 6u && kind == 3u { return vec4f(left.xyz * right.x, 0.0); }
        return visual_componentwise(kind, left, right, opcode);
    }
    if opcode == 7u { return vec4f(select(left.x / right.x, 0.0, abs(right.x) <= 1e-8), 0.0, 0.0, 0.0); }
    if opcode == 8u { return vec4f(abs(left.x), 0.0, 0.0, 0.0); }
    if opcode == 9u { return vec4f(visual_ordered_minimum(left.x, right.x), 0.0, 0.0, 0.0); }
    if opcode == 10u { return vec4f(visual_ordered_maximum(left.x, right.x), 0.0, 0.0, 0.0); }
    if opcode == 11u { return vec4f(visual_ordered_clamp(left.x, right.x, third.x), 0.0, 0.0, 0.0); }
    if opcode == 12u { return vec4f(visual_ordered_step(left.x, right.x), 0.0, 0.0, 0.0); }
    if opcode == 13u {
        if left.x != left.x || right.x != right.x || left.x >= right.x { return vec4f(0.0); }
        return vec4f(smoothstep(left.x, right.x, third.x), 0.0, 0.0, 0.0);
    }
    if opcode == 14u { return vec4f(sin(left.x), 0.0, 0.0, 0.0); }
    if opcode == 15u { return mix(left, right, vec4f(third.x)); }
    if opcode == 16u { return vec4f(select(0.0, 1.0, left.x < right.x), 0.0, 0.0, 0.0); }
    if opcode == 17u { return vec4f(select(0.0, 1.0, left.x > right.x), 0.0, 0.0, 0.0); }
    if opcode == 18u { return vec4f(select(0.0, 1.0, visual_truth(left) && visual_truth(right)), 0.0, 0.0, 0.0); }
    if opcode == 19u { return vec4f(select(0.0, 1.0, visual_truth(left) || visual_truth(right)), 0.0, 0.0, 0.0); }
    if opcode == 20u { return vec4f(select(0.0, 1.0, !visual_truth(left)), 0.0, 0.0, 0.0); }
    if opcode == 21u { return select(third, right, visual_truth(left)); }
    if opcode == 22u { return vec4f(dot(left.xyz, right.xyz), 0.0, 0.0, 0.0); }
    if opcode == 23u {
        let length_squared = dot(left.xyz, left.xyz);
        let invalid = length_squared <= 1e-16
            || length_squared != length_squared
            || abs(length_squared) > 3.402823466e+38;
        return vec4f(select(left.xyz * inverseSqrt(length_squared), vec3f(0.0), invalid), 0.0);
    }
    if opcode == 24u {
        let state_offset = visual_config.arena_offsets.z;
        let state = visual_properties[state_offset + inputs.entity];
        return vec4f(select(0.0, 1.0, (state & bitcast<u32>(instruction.data.x)) != 0u));
    }
    return vec4f(0.0);
}

fn visual_finite_or(value: vec4f, fallback: vec4f) -> vec4f {
    let finite = (value == value) & (abs(value) <= vec4f(3.402823466e+38));
    return select(fallback, value, finite);
}

fn visual_scalar_or(value: f32, fallback: f32) -> f32 {
    return select(fallback, value, value == value && abs(value) <= 3.402823466e+38);
}
