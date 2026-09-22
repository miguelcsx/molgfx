use super::{
    VisualEmitError, emit_resolve, is_specializable, is_supported, opcode_name, unsupported,
};
use crate::representation::visual::{VisualProgram, VisualProgramBuilder};

/// Builds a program, failing loudly when the builder refuses it.
fn build(builder: VisualProgramBuilder) -> VisualProgram {
    builder
        .finish()
        .unwrap_or_else(|error| panic!("program builds: {error}"))
}

/// A program that reads only a constant and a scalar input, so it lowers fully.
fn input_program() -> VisualProgram {
    let mut builder = VisualProgramBuilder::new();
    let opacity = builder
        .base_opacity()
        .unwrap_or_else(|error| panic!("opacity input builds: {error}"));
    let color = builder
        .color([0.1, 0.2, 0.3, 1.0])
        .unwrap_or_else(|error| panic!("color constant builds: {error}"));
    builder
        .set_opacity(opacity)
        .unwrap_or_else(|error| panic!("opacity sets: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("color sets: {error}"));
    build(builder)
}

/// A program that reads an interaction channel, which the emitter rejects.
fn state_program() -> VisualProgram {
    let mut builder = VisualProgramBuilder::new();
    let state = builder
        .interaction_state(1)
        .unwrap_or_else(|error| panic!("state input builds: {error}"));
    let color = builder
        .color([1.0, 1.0, 1.0, 1.0])
        .unwrap_or_else(|error| panic!("color constant builds: {error}"));
    builder
        .set_visibility(state)
        .unwrap_or_else(|error| panic!("visibility sets: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("color sets: {error}"));
    build(builder)
}

/// A program with an arithmetic tail, so several opcodes are exercised.
fn arithmetic_program() -> VisualProgram {
    let mut builder = VisualProgramBuilder::new();
    let distance = builder
        .camera_distance()
        .unwrap_or_else(|error| panic!("distance builds: {error}"));
    let half = builder
        .scalar(0.5)
        .unwrap_or_else(|error| panic!("constant builds: {error}"));
    let scaled = builder
        .multiply(distance, half)
        .unwrap_or_else(|error| panic!("multiply builds: {error}"));
    let clamped = builder
        .saturate(scaled)
        .unwrap_or_else(|error| panic!("clamp builds: {error}"));
    let color = builder
        .color([0.5, 0.5, 0.5, 1.0])
        .unwrap_or_else(|error| panic!("color constant builds: {error}"));
    builder
        .set_opacity(clamped)
        .unwrap_or_else(|error| panic!("opacity sets: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("color sets: {error}"));
    build(builder)
}

#[test]
fn rejected_program_names_the_instruction_and_opcode_that_blocked_it() {
    let program = state_program();
    let error = unsupported(&program).unwrap_or_else(|| panic!("state programs are rejected"));
    let VisualEmitError::UnsupportedOpcode {
        index,
        name,
        opcode,
    } = error
    else {
        panic!("a state instruction reports its opcode, not a size limit");
    };
    assert_eq!(name, "state");
    assert_eq!(opcode, 24);
    let instruction = program
        .instructions()
        .get(index)
        .unwrap_or_else(|| panic!("the reported index names an instruction"));
    assert_eq!(instruction.opcode(), opcode);
    assert!(!is_specializable(&program));
    assert_eq!(
        unsupported(&program).map(|error| error.reason()),
        Some(format!("unsupported-opcode:state@{index}"))
    );
    assert_eq!(
        emit_resolve(&program).err().map(|error| error.reason()),
        Some(format!("unsupported-opcode:state@{index}"))
    );
}

#[test]
fn every_opcode_outside_the_reject_set_is_lowerable() {
    let rejected: Vec<u32> = (0..=24).filter(|opcode| !is_supported(*opcode)).collect();
    assert_eq!(rejected, vec![2, 24]);
    assert_eq!(opcode_name(2), "property");
    assert_eq!(opcode_name(24), "state");
    assert!(is_specializable(&input_program()));
    assert!(is_specializable(&arithmetic_program()));
}

#[test]
fn emitted_body_is_one_function_without_a_register_loop_or_dynamic_dispatch() {
    let program = arithmetic_program();
    let body = emit_resolve(&program).unwrap_or_else(|error| panic!("program lowers: {error}"));

    assert_eq!(body.matches("fn visual_resolve(").count(), 1);
    assert!(
        body.trim_end()
            .ends_with("visual_resolve_registers(&registers, fallback);\n}")
    );
    // The three constructs that make the interpreter an interpreter.
    assert!(!body.contains("for ("));
    assert!(!body.contains("visual_evaluate_instruction"));
    assert!(!body.contains("visual_operand("));
    assert!(!body.contains("counts.x"));
    // The gate and the shared register hand-off are retained verbatim.
    assert!(body.contains("if !VISUAL_FRAGMENT_ENABLED || visual_config.counts.z == 0u {"));
    assert!(body.contains("var registers: array<vec4f, 64>;"));

    // One static assignment per instruction, in program order.
    for index in 0..program.instructions().len() {
        assert!(
            body.contains(&format!("registers[{index}] = ")),
            "instruction {index} must be emitted"
        );
    }
    assert!(
        body.find("registers[0] = ").unwrap_or(usize::MAX)
            < body.find("registers[1] = ").unwrap_or(usize::MAX)
    );
}

#[test]
fn emitted_operands_are_static_register_indices_resolved_at_generation() {
    let program = arithmetic_program();
    let body = emit_resolve(&program).unwrap_or_else(|error| panic!("program lowers: {error}"));
    for instruction in program.instructions() {
        if instruction.opcode() == 0 {
            continue;
        }
        for operand in instruction.operands() {
            assert!(
                body.contains(&format!("registers[{operand}]")),
                "operand {operand} must be emitted as a static index"
            );
        }
    }
}

#[test]
fn normalize_binds_its_squared_length_once_instead_of_repeating_the_dot_product() {
    let mut builder = VisualProgramBuilder::new();
    let seed = builder
        .vector([3.0, 4.0, 0.0])
        .unwrap_or_else(|error| panic!("vector constant builds: {error}"));
    let unit = builder
        .normalize(seed)
        .unwrap_or_else(|error| panic!("normalize builds: {error}"));
    let color = builder
        .color([1.0, 0.0, 0.0, 1.0])
        .unwrap_or_else(|error| panic!("color constant builds: {error}"));
    builder
        .set_position_offset(unit, 1.0)
        .unwrap_or_else(|error| panic!("offset sets: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("color sets: {error}"));
    let program = build(builder);
    let body = emit_resolve(&program).unwrap_or_else(|error| panic!("program lowers: {error}"));
    assert_eq!(body.matches("let length_squared_").count(), 1);
    // The guarded form, not a bare inverse square root.
    assert!(body.contains("inverseSqrt(length_squared_"));
    assert!(body.contains("1e-16"));
}

#[test]
fn scalar_arithmetic_keeps_only_the_first_component_populated() {
    let program = arithmetic_program();
    let body = emit_resolve(&program).unwrap_or_else(|error| panic!("program lowers: {error}"));
    // A scalar multiply must not smear its value across the register.
    assert!(body.contains(".x * "));
    assert!(!body.contains(".xyz * ") || body.contains("visual_input"));
}
