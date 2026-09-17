//! Stable content identity for immutable visual programs.

use super::{Instruction, ProgramOutputs, ValueKind, VisualAttributeRef};

pub(super) fn program_fingerprint(
    instructions: &[Instruction],
    attributes: &[VisualAttributeRef],
    parameter_kinds: &[ValueKind],
    defaults: &[[f32; 4]],
    outputs: ProgramOutputs,
    maximum_displacement: f32,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut write = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for instruction in instructions {
        write(u64::from(instruction.opcode as u32));
        write(u64::from(instruction.kind as u8));
        write(u64::from(instruction.stage as u8));
        for operand in instruction.operands {
            write(u64::from(operand));
        }
        for value in instruction.data {
            write(u64::from(value.to_bits()));
        }
    }
    for attribute in attributes {
        match *attribute {
            VisualAttributeRef::Attribute { handle, kind } => {
                write(0);
                write(u64::from(handle.row()));
                write(u64::from(handle.generation()));
                write(u64::from(kind as u32));
            }
            VisualAttributeRef::LegacyScalar(property) => {
                write(1);
                write(u64::from(property.row()));
                write(u64::from(property.generation()));
            }
            VisualAttributeRef::Column { key, kind } => {
                write(2);
                write(key.0);
                write(u64::from(kind as u32));
            }
        }
    }
    for kind in parameter_kinds {
        write(u64::from(*kind as u8));
    }
    for values in defaults {
        for value in values {
            write(u64::from(value.to_bits()));
        }
    }
    write(u64::from(outputs.present));
    for register in outputs.registers {
        write(u64::from(register));
    }
    write(u64::from(maximum_displacement.to_bits()));
    hash
}
