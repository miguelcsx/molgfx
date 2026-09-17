//! Final validation and low-level register emission for visual builders.

use super::builder::VisualProgramBuilder;
use super::fingerprint::program_fingerprint;
use super::{
    Expr, Instruction, MAX_VISUAL_INSTRUCTIONS, MAX_VISUAL_PARAMETERS, Opcode, Parameter,
    ValueKind, VisualError, VisualOutput, VisualProgram, VisualStage,
};
use std::sync::Arc;

impl VisualProgramBuilder {
    /// Validates and freezes this builder.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program writes no output, when a
    /// geometry-affecting channel is driven by an expression that varies per
    /// fragment, or when validation of the instruction stream fails.
    pub fn finish(mut self) -> Result<VisualProgram, VisualError> {
        if self.outputs.present == 0 {
            return Err(VisualError::MissingOutput);
        }
        for output in [
            VisualOutput::RadiusScale,
            VisualOutput::WidthScale,
            VisualOutput::PositionOffset,
        ] {
            if let Some(register) = self.outputs.get(output)
                && self.instructions[usize::from(register)].stage == VisualStage::Fragment
            {
                return Err(VisualError::StageViolation { output });
            }
        }
        compact_live_instructions(&mut self.instructions, &mut self.outputs);
        let fingerprint = program_fingerprint(
            &self.instructions,
            &self.attributes,
            &self.parameter_kinds,
            &self.parameter_defaults,
            self.outputs,
            self.maximum_displacement,
        );
        Ok(VisualProgram {
            instructions: Arc::from(self.instructions.into_boxed_slice()),
            outputs: self.outputs,
            attributes: Arc::from(self.attributes.into_boxed_slice()),
            properties: Arc::from(self.properties.into_boxed_slice()),
            parameter_kinds: Arc::from(self.parameter_kinds.into_boxed_slice()),
            parameter_defaults: Arc::from(self.parameter_defaults.into_boxed_slice()),
            parameter_owner: self.id,
            maximum_displacement: self.maximum_displacement,
            fingerprint,
        })
    }

    pub(super) fn input(&mut self, kind: ValueKind, input: u8) -> Result<Expr, VisualError> {
        let stage = match input {
            0 | 1 | 8 => VisualStage::Entity,
            2 | 9 | 10 | 11 => VisualStage::Uniform,
            _ => VisualStage::Fragment,
        };
        self.emit_at(
            kind,
            Opcode::Input,
            [0; 3],
            [f32::from(input), 0.0, 0.0, 0.0],
            stage,
        )
    }

    pub(super) fn parameter(
        &mut self,
        kind: ValueKind,
        default: [f32; 4],
    ) -> Result<Parameter, VisualError> {
        if self.parameter_kinds.len() == MAX_VISUAL_PARAMETERS {
            return Err(VisualError::ParameterLimit);
        }
        let index =
            u8::try_from(self.parameter_kinds.len()).map_err(|_| VisualError::ParameterLimit)?;
        self.parameter_kinds.push(kind);
        self.parameter_defaults.push(default);
        Ok(Parameter {
            builder: self.id,
            index,
        })
    }

    pub(super) fn parameter_input(
        &mut self,
        parameter: Parameter,
        kind: ValueKind,
    ) -> Result<Expr, VisualError> {
        self.emit_at(
            kind,
            Opcode::Parameter,
            [0; 3],
            [f32::from(parameter.index), 0.0, 0.0, 0.0],
            VisualStage::Uniform,
        )
    }

    pub(super) fn unary(
        &mut self,
        kind: ValueKind,
        opcode: Opcode,
        value: Expr,
    ) -> Result<Expr, VisualError> {
        self.check(value)?;
        self.emit(kind, opcode, [value.register, 0, 0], [0.0; 4])
    }

    pub(super) fn binary(
        &mut self,
        kind: ValueKind,
        opcode: Opcode,
        left: Expr,
        right: Expr,
    ) -> Result<Expr, VisualError> {
        self.check(left)?;
        self.check(right)?;
        self.emit(kind, opcode, [left.register, right.register, 0], [0.0; 4])
    }

    pub(super) fn ternary(
        &mut self,
        kind: ValueKind,
        opcode: Opcode,
        first: Expr,
        second: Expr,
        third: Expr,
    ) -> Result<Expr, VisualError> {
        self.check(first)?;
        self.check(second)?;
        self.check(third)?;
        self.emit(
            kind,
            opcode,
            [first.register, second.register, third.register],
            [0.0; 4],
        )
    }

    pub(super) fn output(
        &mut self,
        output: VisualOutput,
        expression: Expr,
    ) -> Result<(), VisualError> {
        self.check(expression)?;
        if self.outputs.get(output).is_some() {
            return Err(VisualError::DuplicateOutput { output });
        }
        self.outputs.set(output, expression.register);
        Ok(())
    }

    fn check(&self, expression: Expr) -> Result<(), VisualError> {
        if expression.builder != self.id
            || usize::from(expression.register) >= self.instructions.len()
        {
            Err(VisualError::ForeignExpression)
        } else {
            Ok(())
        }
    }

    fn emit(
        &mut self,
        kind: ValueKind,
        opcode: Opcode,
        operands: [u8; 3],
        data: [f32; 4],
    ) -> Result<Expr, VisualError> {
        let stage = match operands[..opcode.operand_count()]
            .iter()
            .filter_map(|register| self.instructions.get(usize::from(*register)))
            .map(|instruction| instruction.stage)
            .max()
        {
            Some(stage) => stage,
            None => VisualStage::Uniform,
        };
        self.emit_at(kind, opcode, operands, data, stage)
    }

    pub(super) fn emit_at(
        &mut self,
        kind: ValueKind,
        opcode: Opcode,
        operands: [u8; 3],
        data: [f32; 4],
        stage: VisualStage,
    ) -> Result<Expr, VisualError> {
        if self.instructions.len() == MAX_VISUAL_INSTRUCTIONS {
            return Err(VisualError::InstructionLimit);
        }
        let register =
            u8::try_from(self.instructions.len()).map_err(|_| VisualError::InstructionLimit)?;
        self.instructions.push(Instruction {
            opcode,
            kind,
            operands,
            data,
            stage,
        });
        Ok(Expr {
            builder: self.id,
            register,
        })
    }
}

fn compact_live_instructions(
    instructions: &mut Vec<Instruction>,
    outputs: &mut super::ProgramOutputs,
) {
    let mut live = [false; MAX_VISUAL_INSTRUCTIONS];
    for output in ALL_OUTPUTS {
        if let Some(register) = outputs.get(output) {
            live[usize::from(register)] = true;
        }
    }
    for index in (0..instructions.len()).rev() {
        if !live[index] {
            continue;
        }
        let instruction = instructions[index];
        for operand in &instruction.operands[..instruction.opcode.operand_count()] {
            live[usize::from(*operand)] = true;
        }
    }
    let mut remap = [u8::MAX; MAX_VISUAL_INSTRUCTIONS];
    let mut next = 0u8;
    for (index, is_live) in live.iter().copied().enumerate().take(instructions.len()) {
        if is_live {
            remap[index] = next;
            next = next.saturating_add(1);
        }
    }
    for instruction in instructions.iter_mut() {
        for operand in &mut instruction.operands[..instruction.opcode.operand_count()] {
            *operand = remap[usize::from(*operand)];
        }
    }
    let mut index = 0usize;
    instructions.retain(|_| {
        let retain = live[index];
        index += 1;
        retain
    });
    for output in ALL_OUTPUTS {
        if let Some(register) = outputs.get(output) {
            outputs.registers[output as usize] = remap[usize::from(register)];
        }
    }
}

const ALL_OUTPUTS: [VisualOutput; 11] = [
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
];

pub(super) fn finite(values: &[f32]) -> Result<(), VisualError> {
    if values.iter().any(|value| !value.is_finite()) {
        Err(VisualError::NonFinite)
    } else {
        Ok(())
    }
}
