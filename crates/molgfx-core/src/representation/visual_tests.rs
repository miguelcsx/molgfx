use super::{
    Instruction, MAX_VISUAL_INSTRUCTIONS, ValueKind, VisualCompatibility, VisualError,
    VisualInputs, VisualInstructionGpu, VisualOutput, VisualProgram, VisualProgramBuilder,
    VisualStage, VisualStyle,
};
use crate::{
    AtomPropertyHandle, AttributeHandle, AttributeKind, RepresentationKind, ScalarRamp,
    VisualAttributeRef, handle::RawHandle,
};
use molgfx_math::Rgba8;

#[test]
fn property_color_is_entity_staged_and_matches_the_ramp() {
    let property = AtomPropertyHandle(RawHandle::new_for_test(3, 1));
    let ramp = ScalarRamp::sequential([0.0, 10.0]);
    let style = match VisualStyle::color_by_property(property, ramp) {
        Ok(value) => value,
        Err(error) => panic!("property style should build: {error}"),
    };
    assert_eq!(style.program().fragment_instruction_count(), 0);
    let output = style.evaluate(VisualInputs {
        properties: [5.0, f32::NAN, f32::NAN, f32::NAN],
        ..VisualInputs::default()
    });
    let expected = ramp.sample(5.0, Rgba8::WHITE).to_f32();
    for (actual, expected) in output.base_color.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 1.0e-5);
    }
}

#[test]
fn fragment_inputs_do_not_drive_geometry_outputs() {
    let mut builder = VisualProgramBuilder::new();
    let position = match builder.world_position() {
        Ok(value) => value,
        Err(error) => panic!("position input should build: {error}"),
    };
    if let Err(error) = builder.set_position_offset(position, 2.0) {
        panic!("output should remain type-correct until final validation: {error}");
    }
    assert_eq!(
        builder.finish(),
        Err(VisualError::StageViolation {
            output: VisualOutput::PositionOffset,
        })
    );
}

#[test]
fn foreign_expressions_are_rejected_before_they_enter_the_program() {
    let mut first = VisualProgramBuilder::new();
    let mut second = VisualProgramBuilder::new();
    let expression = match first.scalar(1.0) {
        Ok(value) => value,
        Err(error) => panic!("literal should build: {error}"),
    };
    assert_eq!(
        second.set_opacity(expression),
        Err(VisualError::ForeignExpression)
    );
}

#[test]
fn output_channels_are_single_assignment() {
    let mut builder = VisualProgramBuilder::new();
    let first = builder
        .scalar(0.25)
        .unwrap_or_else(|error| panic!("first scalar builds: {error}"));
    let second = builder
        .scalar(0.75)
        .unwrap_or_else(|error| panic!("second scalar builds: {error}"));
    builder
        .set_opacity(first)
        .unwrap_or_else(|error| panic!("first opacity builds: {error}"));
    assert_eq!(
        builder.set_opacity(second),
        Err(VisualError::DuplicateOutput {
            output: VisualOutput::Opacity,
        })
    );
}

#[test]
fn a_color_only_recipe_finishes_without_output_boilerplate() {
    let mut builder = VisualProgramBuilder::new();
    let color = builder
        .color([0.25, 0.5, 0.75, 1.0])
        .unwrap_or_else(|error| panic!("color should build: {error}"));
    let program = builder
        .finish_color(color)
        .unwrap_or_else(|error| panic!("color recipe should finish: {error}"));

    assert_eq!(
        program
            .evaluate(VisualInputs::default())
            .base_color
            .map(f32::to_bits),
        [0.25, 0.5, 0.75, 1.0].map(f32::to_bits)
    );
}

#[test]
fn finishing_removes_dead_instructions_and_remaps_dependencies() {
    let mut builder = VisualProgramBuilder::new();
    builder
        .scalar(99.0)
        .unwrap_or_else(|error| panic!("dead scalar should build: {error}"));
    let left = builder
        .scalar(0.25)
        .unwrap_or_else(|error| panic!("left scalar should build: {error}"));
    builder
        .scalar(77.0)
        .unwrap_or_else(|error| panic!("second dead scalar should build: {error}"));
    let right = builder
        .scalar(0.5)
        .unwrap_or_else(|error| panic!("right scalar should build: {error}"));
    let opacity = builder
        .add(left, right)
        .unwrap_or_else(|error| panic!("sum should build: {error}"));
    builder
        .set_opacity(opacity)
        .unwrap_or_else(|error| panic!("opacity should build: {error}"));
    let program = builder
        .finish()
        .unwrap_or_else(|error| panic!("program should finish: {error}"));

    assert_eq!(program.instructions().len(), 3);
    assert_eq!(program.output_register(VisualOutput::Opacity), Some(2));
    assert_eq!(
        program.evaluate(VisualInputs::default()).opacity.to_bits(),
        0.75f32.to_bits()
    );
}

#[test]
fn parameter_updates_reuse_the_same_program() {
    let mut builder = VisualProgramBuilder::new();
    let (parameter, expression) = match builder.scalar_parameter(0.25) {
        Ok(value) => value,
        Err(error) => panic!("parameter should build: {error}"),
    };
    if let Err(error) = builder.set_opacity(expression) {
        panic!("opacity output should build: {error}");
    }
    let mut style = match builder.finish() {
        Ok(program) => VisualStyle::new(program),
        Err(error) => panic!("program should finish: {error}"),
    };
    let fingerprint = style.program().fingerprint();
    if let Err(error) = style.set_scalar(parameter, 0.75) {
        panic!("parameter should update: {error}");
    }
    assert_eq!(style.program().fingerprint(), fingerprint);
    let opacity = style.evaluate(VisualInputs::default()).opacity;
    assert!((opacity - 0.75).abs() < 1e-6, "opacity was {opacity}");
}

#[test]
fn programs_stop_at_the_portable_instruction_limit() {
    let mut builder = VisualProgramBuilder::new();
    for _ in 0..MAX_VISUAL_INSTRUCTIONS {
        if let Err(error) = builder.scalar(1.0) {
            panic!("instruction inside limit should build: {error}");
        }
    }
    assert_eq!(builder.scalar(1.0), Err(VisualError::InstructionLimit));
}

#[test]
fn implicit_surfaces_reject_bounded_displacement() {
    let mut builder = VisualProgramBuilder::new();
    let displacement = match builder.vector([1.0, 0.0, 0.0]) {
        Ok(value) => value,
        Err(error) => panic!("vector should build: {error}"),
    };
    if let Err(error) = builder.set_position_offset(displacement, 1.0) {
        panic!("bounded displacement should build: {error}");
    }
    let program = match builder.finish() {
        Ok(value) => value,
        Err(error) => panic!("program should finish: {error}"),
    };
    assert_eq!(program.instructions()[0].stage(), VisualStage::Uniform);
    assert_eq!(
        program.validate_compatibility(VisualCompatibility::for_representation(
            RepresentationKind::Surface,
        )),
        Err(VisualError::UnsupportedOutput {
            output: VisualOutput::PositionOffset,
        })
    );
}

#[test]
fn representations_without_native_visual_lowering_reject_styles() {
    let mut builder = VisualProgramBuilder::new();
    let color = builder
        .color([0.2, 0.4, 0.6, 1.0])
        .unwrap_or_else(|error| panic!("color should build: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("output should build: {error}"));
    let program = builder
        .finish()
        .unwrap_or_else(|error| panic!("program should build: {error}"));

    for kind in [RepresentationKind::Volume, RepresentationKind::Segmentation] {
        assert_eq!(
            program.validate_compatibility(VisualCompatibility::for_representation(kind)),
            Err(VisualError::UnsupportedOutput {
                output: VisualOutput::BaseColor,
            })
        );
    }

    let mut width_builder = VisualProgramBuilder::new();
    let width = width_builder
        .scalar(2.0)
        .unwrap_or_else(|error| panic!("width should build: {error}"));
    width_builder
        .set_width_scale(width)
        .unwrap_or_else(|error| panic!("width output should build: {error}"));
    let width_program = width_builder
        .finish()
        .unwrap_or_else(|error| panic!("width program should build: {error}"));
    assert_eq!(
        width_program.validate_compatibility(VisualCompatibility::for_representation(
            RepresentationKind::Cartoon,
        )),
        Err(VisualError::UnsupportedOutput {
            output: VisualOutput::WidthScale,
        })
    );

    let mut softness_builder = VisualProgramBuilder::new();
    let softness = softness_builder
        .scalar(2.0)
        .unwrap_or_else(|error| panic!("softness should build: {error}"));
    softness_builder
        .set_silhouette_softness(softness)
        .unwrap_or_else(|error| panic!("softness output should build: {error}"));
    let softness_program = softness_builder
        .finish()
        .unwrap_or_else(|error| panic!("softness program should build: {error}"));
    assert_eq!(
        softness_program.validate_compatibility(VisualCompatibility::RIBBON),
        Err(VisualError::UnsupportedOutput {
            output: VisualOutput::SilhouetteSoftness,
        })
    );
}

#[test]
fn gpu_instruction_lowering_is_fixed_width_and_reuses_storage() {
    let mut builder = VisualProgramBuilder::new();
    let color = builder
        .color([0.2, 0.4, 0.6, 1.0])
        .unwrap_or_else(|error| panic!("color builds: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("output builds: {error}"));
    let program = builder
        .finish()
        .unwrap_or_else(|error| panic!("program builds: {error}"));
    let mut records = Vec::with_capacity(MAX_VISUAL_INSTRUCTIONS);
    program.write_gpu_instructions(&mut records);
    let pointer = records.as_ptr();
    program.write_gpu_instructions(&mut records);
    assert_eq!(std::mem::size_of::<VisualInstructionGpu>(), 32);
    assert_eq!(records.as_ptr(), pointer);
    assert_eq!(records[0].control[0], 0);
}

#[test]
fn serialized_programs_reject_forged_types_and_stages() {
    let scalar =
        Instruction::from_serialized(0, ValueKind::Scalar.code(), [0; 3], [1.0, 0.0, 0.0, 0.0], 0)
            .unwrap_or_else(|error| panic!("instruction decodes: {error}"));
    assert_eq!(
        VisualProgram::from_serialized(
            vec![scalar],
            &[(VisualOutput::BaseColor, 0)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            0.0,
        ),
        Err(VisualError::MalformedProgram),
    );

    let forged_stage =
        Instruction::from_serialized(1, ValueKind::Vector.code(), [0; 3], [3.0, 0.0, 0.0, 0.0], 0)
            .unwrap_or_else(|error| panic!("instruction decodes: {error}"));
    assert_eq!(
        VisualProgram::from_serialized(
            vec![forged_stage],
            &[(VisualOutput::PositionOffset, 0)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            1.0,
        ),
        Err(VisualError::MalformedProgram),
    );
}

#[test]
fn malformed_runtime_ranges_and_non_finite_inputs_resolve_without_panicking() {
    let property = AtomPropertyHandle(RawHandle::new_for_test(5, 1));
    let mut builder = VisualProgramBuilder::new();
    let value = builder
        .atom_property(property)
        .unwrap_or_else(|error| panic!("property should build: {error}"));
    let low = builder
        .scalar(2.0)
        .unwrap_or_else(|error| panic!("low bound should build: {error}"));
    let high = builder
        .scalar(1.0)
        .unwrap_or_else(|error| panic!("high bound should build: {error}"));
    let clamped = builder
        .clamp(value, low, high)
        .unwrap_or_else(|error| panic!("dynamic clamp should build: {error}"));
    builder
        .set_opacity(clamped)
        .unwrap_or_else(|error| panic!("opacity should build: {error}"));
    let program = builder
        .finish()
        .unwrap_or_else(|error| panic!("program should build: {error}"));

    let evaluation = program.evaluate(VisualInputs {
        base_opacity: 0.375,
        properties: [f32::NAN; 4],
        ..VisualInputs::default()
    });
    assert_eq!(evaluation.opacity.to_bits(), 0.375f32.to_bits());
}

#[test]
fn non_finite_boolean_and_vector_inputs_have_deterministic_fallbacks() {
    let mut visibility_builder = VisualProgramBuilder::new();
    let entity = visibility_builder
        .entity_index()
        .unwrap_or_else(|error| panic!("entity input should build: {error}"));
    let zero = visibility_builder
        .scalar(0.0)
        .unwrap_or_else(|error| panic!("zero should build: {error}"));
    let visible = visibility_builder
        .greater(entity, zero)
        .unwrap_or_else(|error| panic!("comparison should build: {error}"));
    visibility_builder
        .set_visibility(visible)
        .unwrap_or_else(|error| panic!("visibility should build: {error}"));
    let visibility = visibility_builder
        .finish()
        .unwrap_or_else(|error| panic!("visibility program should build: {error}"));
    assert!(!visibility.evaluate(VisualInputs::default()).visible);

    let mut normal_builder = VisualProgramBuilder::new();
    let local = normal_builder
        .local_position()
        .unwrap_or_else(|error| panic!("local position should build: {error}"));
    let normalized = normal_builder
        .normalize(local)
        .unwrap_or_else(|error| panic!("normalization should build: {error}"));
    let axis = normal_builder
        .vector([1.0, 0.0, 0.0])
        .unwrap_or_else(|error| panic!("axis should build: {error}"));
    let response = normal_builder
        .dot(normalized, axis)
        .unwrap_or_else(|error| panic!("dot should build: {error}"));
    normal_builder
        .set_roughness(response)
        .unwrap_or_else(|error| panic!("roughness should build: {error}"));
    let normal_program = normal_builder
        .finish()
        .unwrap_or_else(|error| panic!("normal program should build: {error}"));
    let evaluation = normal_program.evaluate(VisualInputs {
        local_position: [f32::MAX; 3],
        roughness: 0.4,
        ..VisualInputs::default()
    });
    assert_eq!(evaluation.roughness.to_bits(), 0.05f32.to_bits());
}

#[test]
fn typed_attributes_share_four_slots_and_preserve_value_kinds() {
    let scalar = AttributeHandle(RawHandle::new_for_test(4, 1));
    let vector = AttributeHandle(RawHandle::new_for_test(5, 1));
    let color = AttributeHandle(RawHandle::new_for_test(6, 1));
    let mut builder = VisualProgramBuilder::new();
    let scalar_value = builder.scalar_attribute(scalar).unwrap();
    let vector_value = builder.vector_attribute(vector).unwrap();
    let color_value = builder.color_attribute(color).unwrap();
    let axis = builder.vector([1.0, 0.0, 0.0]).unwrap();
    let projection = builder.dot(vector_value, axis).unwrap();
    let opacity = builder.multiply(scalar_value, projection).unwrap();
    builder.set_base_color(color_value).unwrap();
    builder.set_opacity(opacity).unwrap();
    let program = builder.finish().unwrap();

    assert_eq!(
        program.attributes(),
        &[
            VisualAttributeRef::Attribute {
                handle: scalar,
                kind: AttributeKind::Scalar,
            },
            VisualAttributeRef::Attribute {
                handle: vector,
                kind: AttributeKind::Vector,
            },
            VisualAttributeRef::Attribute {
                handle: color,
                kind: AttributeKind::Color,
            },
        ]
    );
    let evaluation = program.evaluate(VisualInputs {
        attributes: [
            [0.5, 0.0, 0.0, 0.0],
            [0.75, 1.0, 2.0, 0.0],
            [0.2, 0.4, 0.6, 1.0],
            [f32::NAN; 4],
        ],
        ..VisualInputs::default()
    });
    assert_eq!(evaluation.opacity.to_bits(), 0.375f32.to_bits());
    assert_eq!(
        evaluation.base_color.map(f32::to_bits),
        [0.2, 0.4, 0.6, 1.0].map(f32::to_bits)
    );
}
