// Visual-program configuration and selective result-lane layout.

fn set_outputs(program: &pdviewx_core::VisualProgram, config: &mut VisualConfig) {
    let set = |output: VisualOutput, register: &mut u32| {
        *register = program
            .output_register(output)
            .map_or(MISSING_REGISTER, u32::from);
    };
    set(VisualOutput::BaseColor, &mut config.outputs0[0]);
    set(VisualOutput::Opacity, &mut config.outputs0[1]);
    set(VisualOutput::Emission, &mut config.outputs0[2]);
    set(VisualOutput::Roughness, &mut config.outputs0[3]);
    set(VisualOutput::Specular, &mut config.outputs1[0]);
    set(VisualOutput::MaterialStrength, &mut config.outputs1[1]);
    set(VisualOutput::Visibility, &mut config.outputs1[2]);
    set(VisualOutput::SilhouetteSoftness, &mut config.outputs1[3]);
    set(VisualOutput::RadiusScale, &mut config.outputs2[0]);
    set(VisualOutput::WidthScale, &mut config.outputs2[1]);
    set(VisualOutput::PositionOffset, &mut config.outputs2[2]);
}

fn styled_config(input: &VisualConfigInput<'_>) -> VisualConfig {
    let base_color = input.base.color;
    let base_material = input.base.material;
    let uniform = input.style.evaluate(pdviewx_core::VisualInputs {
        base_color,
        base_opacity: base_material[0],
        time_seconds: input.time_seconds,
        roughness: base_material[1],
        specular: base_material[2],
        material_strength: base_material[3],
        ..pdviewx_core::VisualInputs::default()
    });
    let mut config = VisualConfig {
        counts: [
            saturating_u32(input.program.instructions().len()),
            saturating_u32(input.entity_count),
            saturating_u32(input.program.fragment_instruction_count()),
            saturating_u32(input.style.parameters().len()),
        ],
        base_color,
        material: base_material,
        uniform_emission: [0.0; 4],
        uniform_geometry: [1.0, 0.0, 0.25, 0.25],
        uniform_offset: [0.0; 4],
        presentation: [
            input.time_seconds,
            input.program.maximum_displacement(),
            0.0,
            0.0,
        ],
        property_offsets: input.property_offsets,
        attribute_layouts: input.attribute_layouts,
        property_end_offsets: input.property_end_offsets,
        property_alphas: input.property_alphas,
        arena_offsets: [input.program_offset, input.parameter_offset, 0, 0],
        result_layout: result_layout(input.program, input.result_count),
        ..VisualConfig::default()
    };
    apply_uniform_outputs(input.program, uniform, &mut config);
    set_outputs(input.program, &mut config);
    config
}

fn apply_uniform_outputs(
    program: &pdviewx_core::VisualProgram,
    value: pdviewx_core::VisualEvaluation,
    config: &mut VisualConfig,
) {
    if uniform_output(program, VisualOutput::BaseColor) {
        config.base_color[..3].copy_from_slice(&value.base_color[..3]);
    }
    if uniform_output(program, VisualOutput::Opacity) {
        config.base_color[3] = value.opacity;
        config.material[0] = value.opacity;
    }
    if uniform_output(program, VisualOutput::Emission) {
        config.uniform_emission[..3].copy_from_slice(&value.emission);
    }
    if uniform_output(program, VisualOutput::Roughness) {
        config.material[1] = value.roughness;
    }
    if uniform_output(program, VisualOutput::Specular) {
        config.material[2] = value.specular;
    }
    if uniform_output(program, VisualOutput::MaterialStrength) {
        config.material[3] = value.material_strength;
    }
    if uniform_output(program, VisualOutput::Visibility) {
        config.uniform_geometry[0] = f32::from(u8::from(value.visible));
    }
    if uniform_output(program, VisualOutput::SilhouetteSoftness) {
        config.uniform_geometry[1] = value.silhouette_softness / 8.0;
    }
    if uniform_output(program, VisualOutput::RadiusScale) {
        config.uniform_geometry[2] = value.radius_scale / 4.0;
    }
    if uniform_output(program, VisualOutput::WidthScale) {
        config.uniform_geometry[3] = value.width_scale / 4.0;
    }
    if uniform_output(program, VisualOutput::PositionOffset) {
        config.uniform_offset[..3].copy_from_slice(&value.position_offset);
    }
}

fn uniform_output(program: &pdviewx_core::VisualProgram, output: VisualOutput) -> bool {
    program.output_stage(output) == Some(pdviewx_core::VisualStage::Uniform)
}

const RESULT_COLOR: u32 = 1 << 0;
const RESULT_EMISSION: u32 = 1 << 1;
const RESULT_RESPONSE: u32 = 1 << 2;
const RESULT_GEOMETRY: u32 = 1 << 3;
const RESULT_OFFSET: u32 = 1 << 4;

fn result_layout(program: &pdviewx_core::VisualProgram, count: usize) -> [u32; 4] {
    let entity_lane = |outputs: &[VisualOutput]| {
        outputs.iter().any(|output| {
            program
                .output_stage(*output)
                .is_some_and(|stage| stage == pdviewx_core::VisualStage::Entity)
        })
    };
    let mut mask = 0;
    mask |= u32::from(entity_lane(&[
        VisualOutput::BaseColor,
        VisualOutput::Opacity,
    ])) * RESULT_COLOR;
    mask |= u32::from(entity_lane(&[VisualOutput::Emission])) * RESULT_EMISSION;
    mask |= u32::from(entity_lane(&[
        VisualOutput::Roughness,
        VisualOutput::Specular,
        VisualOutput::MaterialStrength,
    ])) * RESULT_RESPONSE;
    mask |= u32::from(entity_lane(&[
        VisualOutput::Visibility,
        VisualOutput::SilhouetteSoftness,
        VisualOutput::RadiusScale,
        VisualOutput::WidthScale,
    ])) * RESULT_GEOMETRY;
    mask |= u32::from(entity_lane(&[VisualOutput::PositionOffset])) * RESULT_OFFSET;
    let count = saturating_u32(count);
    let packed_words = (mask & 0x0f).count_ones().saturating_mul(count);
    let offset_base = if mask & RESULT_OFFSET == 0 {
        u32::MAX
    } else {
        packed_words
    };
    let mut uniform_mask = 0;
    for output in [
        VisualOutput::BaseColor,
        VisualOutput::Opacity,
        VisualOutput::Emission,
        VisualOutput::Roughness,
        VisualOutput::Specular,
        VisualOutput::MaterialStrength,
        VisualOutput::Visibility,
        VisualOutput::SilhouetteSoftness,
        VisualOutput::RadiusScale,
        VisualOutput::WidthScale,
        VisualOutput::PositionOffset,
    ] {
        uniform_mask |= u32::from(uniform_output(program, output)) * (1 << output as u32);
    }
    [mask, uniform_mask, offset_base, count]
}
