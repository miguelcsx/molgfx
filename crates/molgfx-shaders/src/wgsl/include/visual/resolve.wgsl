// Shared fragment-output resolution for every drawable family.

struct VisualFragmentResult {
    color: vec4f,
    emission: vec3f,
    emission_enabled: bool,
    roughness: f32,
    specular: f32,
    material_strength: f32,
    visible: bool,
    softness_pixels: f32,
}

fn visual_fragment_value(
    registers: ptr<function, array<vec4f, 64>>,
    slot: u32,
    fallback: vec4f,
) -> vec4f {
    if slot == VISUAL_MISSING || visual_instruction(slot).control.w != 2u {
        return fallback;
    }
    return (*registers)[slot];
}

fn visual_resolve(
    inputs: VisualEvaluationInputs,
    fallback: VisualFragmentResult,
) -> VisualFragmentResult {
    if !VISUAL_FRAGMENT_ENABLED || visual_config.counts.z == 0u {
        return fallback;
    }
    var registers: array<vec4f, 64>;
    for (var index = 0u; index < visual_config.counts.x; index++) {
        registers[index] = visual_evaluate_instruction(
            visual_instruction(index),
            inputs,
            &registers,
        );
    }
    let color = clamp(
        visual_finite_or(
            visual_fragment_value(&registers, visual_config.outputs0.x, fallback.color),
            fallback.color,
        ),
        vec4f(0.0),
        vec4f(1.0),
    );
    let opacity = clamp(
        visual_scalar_or(
            visual_fragment_value(&registers, visual_config.outputs0.y, fallback.color).x,
            fallback.color.a,
        ),
        0.0,
        1.0,
    );
    let emission = clamp(
        visual_finite_or(
            visual_fragment_value(&registers, visual_config.outputs0.z, vec4f(fallback.emission, 0.0)),
            vec4f(fallback.emission, 0.0),
        ).rgb,
        vec3f(0.0),
        vec3f(64.0),
    );
    let roughness = clamp(
        visual_scalar_or(
            visual_fragment_value(&registers, visual_config.outputs0.w, vec4f(fallback.roughness)).x,
            fallback.roughness,
        ),
        0.05,
        0.92,
    );
    let specular = clamp(
        visual_scalar_or(
            visual_fragment_value(&registers, visual_config.outputs1.x, vec4f(fallback.specular)).x,
            fallback.specular,
        ),
        0.0,
        1.0,
    );
    let strength = clamp(
        visual_scalar_or(
            visual_fragment_value(&registers, visual_config.outputs1.y, vec4f(fallback.material_strength)).x,
            fallback.material_strength,
        ),
        0.0,
        1.0,
    );
    let visible = visual_truth(
        visual_fragment_value(
            &registers,
            visual_config.outputs1.z,
            vec4f(select(0.0, 1.0, fallback.visible)),
        ),
    );
    let softness = clamp(
        visual_scalar_or(
            visual_fragment_value(&registers, visual_config.outputs1.w, vec4f(fallback.softness_pixels)).x,
            fallback.softness_pixels,
        ),
        0.0,
        8.0,
    );
    return VisualFragmentResult(
        vec4f(color.rgb, opacity),
        emission,
        visual_config.outputs0.z != VISUAL_MISSING,
        roughness,
        specular,
        strength,
        visible,
        softness,
    );
}
