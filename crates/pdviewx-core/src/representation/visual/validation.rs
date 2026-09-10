//! Validation of cold serialized instruction streams.

use super::{Instruction, Opcode, ValueKind, VisualError, VisualOutput, VisualStage};

pub(super) fn validate_instructions(
    instructions: &[Instruction],
    property_count: usize,
    parameter_kinds: &[ValueKind],
) -> Result<(), VisualError> {
    for (index, instruction) in instructions.iter().enumerate() {
        for operand in &instruction.operands[..instruction.opcode.operand_count()] {
            if usize::from(*operand) >= index {
                return Err(VisualError::MalformedProgram);
            }
        }
        let expected =
            validate_instruction(instruction, instructions, property_count, parameter_kinds)?;
        if instruction.stage != expected {
            return Err(VisualError::MalformedProgram);
        }
    }
    Ok(())
}

fn validate_instruction(
    instruction: &Instruction,
    instructions: &[Instruction],
    property_count: usize,
    parameter_kinds: &[ValueKind],
) -> Result<VisualStage, VisualError> {
    let kind = |slot: usize| instructions[usize::from(instruction.operands[slot])].kind;
    let matches = |expected: &[ValueKind]| {
        expected
            .iter()
            .enumerate()
            .all(|(slot, expected)| kind(slot) == *expected)
    };
    let valid = match instruction.opcode {
        Opcode::Constant => true,
        Opcode::Input => {
            input_contract(instruction.data[0]).is_some_and(|value| value.0 == instruction.kind)
        }
        Opcode::Property => {
            matches!(
                instruction.kind,
                ValueKind::Scalar | ValueKind::Vector | ValueKind::Color
            ) && exact_index(instruction.data[0]).is_some_and(|slot| slot < property_count)
                && exact_index(instruction.data[1]).is_some_and(|source| source <= 1)
        }
        Opcode::Parameter => {
            exact_index(instruction.data[0]).and_then(|slot| parameter_kinds.get(slot))
                == Some(&instruction.kind)
        }
        Opcode::Add | Opcode::Subtract => {
            matches!(instruction.kind, ValueKind::Scalar | ValueKind::Vector)
                && matches(&[instruction.kind, instruction.kind])
        }
        Opcode::Multiply => match instruction.kind {
            ValueKind::Scalar => matches(&[ValueKind::Scalar, ValueKind::Scalar]),
            ValueKind::Vector => matches(&[ValueKind::Vector, ValueKind::Scalar]),
            ValueKind::Color | ValueKind::Bool => false,
        },
        Opcode::SafeDivide | Opcode::Minimum | Opcode::Maximum | Opcode::Step => {
            instruction.kind == ValueKind::Scalar
                && matches(&[ValueKind::Scalar, ValueKind::Scalar])
        }
        Opcode::Abs | Opcode::Sine => {
            instruction.kind == ValueKind::Scalar && matches(&[ValueKind::Scalar])
        }
        Opcode::Clamp | Opcode::SmoothStep => {
            instruction.kind == ValueKind::Scalar
                && matches(&[ValueKind::Scalar, ValueKind::Scalar, ValueKind::Scalar])
        }
        Opcode::Mix => {
            matches!(instruction.kind, ValueKind::Scalar | ValueKind::Color)
                && matches(&[instruction.kind, instruction.kind, ValueKind::Scalar])
        }
        Opcode::Less | Opcode::Greater => {
            instruction.kind == ValueKind::Bool && matches(&[ValueKind::Scalar, ValueKind::Scalar])
        }
        Opcode::And | Opcode::Or => {
            instruction.kind == ValueKind::Bool && matches(&[ValueKind::Bool, ValueKind::Bool])
        }
        Opcode::Not => instruction.kind == ValueKind::Bool && matches(&[ValueKind::Bool]),
        Opcode::Select => matches(&[ValueKind::Bool, instruction.kind, instruction.kind]),
        Opcode::Dot => {
            instruction.kind == ValueKind::Scalar
                && matches(&[ValueKind::Vector, ValueKind::Vector])
        }
        Opcode::Normalize => instruction.kind == ValueKind::Vector && matches(&[ValueKind::Vector]),
    };
    if !valid {
        return Err(VisualError::MalformedProgram);
    }
    Ok(match instruction.opcode {
        Opcode::Constant | Opcode::Parameter => VisualStage::Uniform,
        Opcode::Property => VisualStage::Entity,
        Opcode::Input => input_contract(instruction.data[0])
            .map(|value| value.1)
            .ok_or(VisualError::MalformedProgram)?,
        _ => match instruction.operands[..instruction.opcode.operand_count()]
            .iter()
            .map(|operand| instructions[usize::from(*operand)].stage)
            .max()
        {
            Some(stage) => stage,
            None => VisualStage::Uniform,
        },
    })
}

/// A payload float that names a slot, when it really is a whole index.
///
/// Only exactly-representable non-negative whole numbers name a slot; anything
/// else came from a corrupt or hand-built program and is rejected here rather
/// than silently truncated into a valid-looking index.
fn exact_index(value: f32) -> Option<usize> {
    if value < 0.0 || value.fract() != 0.0 {
        return None;
    }
    Some(super::numeric::decode_slot(value))
}

fn input_contract(value: f32) -> Option<(ValueKind, VisualStage)> {
    match exact_index(value)? {
        0 => Some((ValueKind::Color, VisualStage::Entity)),
        1 | 8 => Some((ValueKind::Scalar, VisualStage::Entity)),
        2 | 9 | 10 | 11 => Some((ValueKind::Scalar, VisualStage::Uniform)),
        3..=6 => Some((ValueKind::Vector, VisualStage::Fragment)),
        7 => Some((ValueKind::Scalar, VisualStage::Fragment)),
        _ => None,
    }
}

pub(super) const fn output_kind(output: VisualOutput) -> ValueKind {
    match output {
        VisualOutput::BaseColor | VisualOutput::Emission => ValueKind::Color,
        VisualOutput::Visibility => ValueKind::Bool,
        VisualOutput::PositionOffset => ValueKind::Vector,
        VisualOutput::Opacity
        | VisualOutput::Roughness
        | VisualOutput::Specular
        | VisualOutput::MaterialStrength
        | VisualOutput::SilhouetteSoftness
        | VisualOutput::RadiusScale
        | VisualOutput::WidthScale => ValueKind::Scalar,
    }
}
