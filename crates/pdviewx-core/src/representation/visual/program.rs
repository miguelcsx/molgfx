//! Immutable visual programs, parameter blocks and cold-stream validation.

use super::fingerprint::program_fingerprint;
use super::types::{
    ColorParameter, Instruction, Parameter, ProgramOutputs, ScalarParameter, ValueKind,
    VectorParameter, VisualCompatibility, VisualError, VisualOutput, VisualStage,
    next_program_owner,
};
use super::{MAX_VISUAL_INSTRUCTIONS, MAX_VISUAL_PARAMETERS, MAX_VISUAL_PROPERTIES};
use crate::{AtomPropertyHandle, AttributeHandle, AttributeKind};
use std::fmt;
use std::sync::Arc;

/// Scene-independent identity for one caller-owned paged visual column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct VisualColumnKey(pub u64);

/// Fixed 32-byte instruction record uploaded to persistent GPU arenas.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VisualInstructionGpu {
    /// Opcode, value kind, packed operands and earliest stage.
    pub control: [u32; 4],
    /// Literal payload or input/descriptor metadata.
    pub data: [f32; 4],
}

/// One typed column slot referenced by a visual program.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum VisualAttributeRef {
    /// Schema-8 generic typed attribute.
    Attribute {
        /// Stable scene handle.
        handle: AttributeHandle,
        /// Physical kind expected by the typed expression.
        kind: AttributeKind,
    },
    /// Scene-independent column bound to an exact residency ticket by a
    /// chunk visual descriptor.
    Column {
        /// Stable caller-owned column identity.
        key: VisualColumnKey,
        /// Native physical kind expected by the program.
        kind: AttributeKind,
    },
    /// Pre-schema-8 scalar source retained only while old manifests migrate.
    #[doc(hidden)]
    LegacyScalar(AtomPropertyHandle),
}

impl VisualAttributeRef {
    /// Physical layout expected by this slot.
    #[must_use]
    pub const fn kind(self) -> AttributeKind {
        match self {
            Self::Attribute { kind, .. } | Self::Column { kind, .. } => kind,
            Self::LegacyScalar(_) => AttributeKind::Scalar,
        }
    }

    /// Generic attribute handle when this is a schema-8 slot.
    #[must_use]
    pub const fn attribute(self) -> Option<AttributeHandle> {
        match self {
            Self::Attribute { handle, .. } => Some(handle),
            Self::Column { .. } | Self::LegacyScalar(_) => None,
        }
    }

    /// Scene-independent column identity, when this program is paged.
    #[must_use]
    pub const fn column(self) -> Option<VisualColumnKey> {
        match self {
            Self::Column { key, .. } => Some(key),
            Self::Attribute { .. } | Self::LegacyScalar(_) => None,
        }
    }

    /// Transitional scalar handle, absent for generic attributes.
    #[doc(hidden)]
    #[must_use]
    pub const fn legacy_scalar(self) -> Option<AtomPropertyHandle> {
        match self {
            Self::Attribute { .. } | Self::Column { .. } => None,
            Self::LegacyScalar(handle) => Some(handle),
        }
    }
}

/// Immutable validated visual program.
#[derive(Clone, Debug)]
pub struct VisualProgram {
    pub(crate) instructions: Arc<[Instruction]>,
    pub(crate) outputs: ProgramOutputs,
    pub(crate) attributes: Arc<[VisualAttributeRef]>,
    pub(crate) properties: Arc<[AtomPropertyHandle]>,
    pub(crate) parameter_kinds: Arc<[ValueKind]>,
    pub(crate) parameter_defaults: Arc<[[f32; 4]]>,
    pub(crate) parameter_owner: u64,
    pub(crate) maximum_displacement: f32,
    pub(crate) fingerprint: u64,
}

impl PartialEq for VisualProgram {
    fn eq(&self, other: &Self) -> bool {
        self.instructions == other.instructions
            && self.outputs == other.outputs
            && self.attributes == other.attributes
            && self.properties == other.properties
            && self.parameter_kinds == other.parameter_kinds
            && self.parameter_defaults == other.parameter_defaults
            && self.maximum_displacement == other.maximum_displacement
            && self.fingerprint == other.fingerprint
    }
}

impl VisualProgram {
    /// Compiled instruction stream, stable for the program lifetime.
    #[must_use]
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
    /// Referenced typed columns in instruction-slot order.
    #[must_use]
    pub fn attributes(&self) -> &[VisualAttributeRef] {
        &self.attributes
    }
    /// Transitional atom-property columns used by old scene descriptions.
    #[doc(hidden)]
    #[must_use]
    pub fn properties(&self) -> &[AtomPropertyHandle] {
        &self.properties
    }
    /// Declared conservative local displacement bound.
    #[must_use]
    pub const fn maximum_displacement(&self) -> f32 {
        self.maximum_displacement
    }
    /// Deterministic content fingerprint used for GPU deduplication.
    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Writes GPU-ready fixed-width instructions into caller-reused storage.
    pub fn write_gpu_instructions(&self, output: &mut Vec<VisualInstructionGpu>) {
        output.clear();
        output.reserve(self.instructions.len().saturating_sub(output.capacity()));
        output.extend(self.instructions.iter().map(|instruction| {
            let operands = instruction.operands;
            VisualInstructionGpu {
                control: [
                    instruction.opcode(),
                    u32::from(instruction.kind_code()),
                    u32::from(operands[0])
                        | (u32::from(operands[1]) << 8)
                        | (u32::from(operands[2]) << 16),
                    u32::from(instruction.stage().code()),
                ],
                data: instruction.data(),
            }
        }));
    }

    /// Register written by a channel, when the channel is present.
    #[must_use]
    pub const fn output_register(&self, output: VisualOutput) -> Option<u8> {
        self.outputs.get(output)
    }

    /// Earliest stage needed by one output channel.
    #[must_use]
    pub fn output_stage(&self, output: VisualOutput) -> Option<VisualStage> {
        self.output_register(output)
            .and_then(|register| self.instructions.get(usize::from(register)))
            .map(Instruction::stage)
    }
    pub(crate) fn parameter_kinds(&self) -> &[ValueKind] {
        &self.parameter_kinds
    }
    pub(crate) fn parameter_defaults(&self) -> &[[f32; 4]] {
        &self.parameter_defaults
    }

    #[cfg(test)]
    pub(crate) fn from_serialized(
        instructions: Vec<Instruction>,
        outputs: &[(VisualOutput, u8)],
        properties: Vec<AtomPropertyHandle>,
        parameter_kinds: Vec<ValueKind>,
        parameter_defaults: Vec<[f32; 4]>,
        maximum_displacement: f32,
    ) -> Result<Self, VisualError> {
        let attributes = properties
            .iter()
            .copied()
            .map(VisualAttributeRef::LegacyScalar)
            .collect::<Vec<_>>();
        Self::from_serialized_attributes(
            instructions,
            outputs,
            attributes,
            properties,
            parameter_kinds,
            parameter_defaults,
            maximum_displacement,
        )
    }

    pub(crate) fn from_serialized_attributes(
        instructions: Vec<Instruction>,
        outputs: &[(VisualOutput, u8)],
        attributes: Vec<VisualAttributeRef>,
        properties: Vec<AtomPropertyHandle>,
        parameter_kinds: Vec<ValueKind>,
        parameter_defaults: Vec<[f32; 4]>,
        maximum_displacement: f32,
    ) -> Result<Self, VisualError> {
        if instructions.is_empty()
            || instructions.len() > MAX_VISUAL_INSTRUCTIONS
            || attributes.len() > MAX_VISUAL_PROPERTIES
            || parameter_kinds.len() > MAX_VISUAL_PARAMETERS
            || parameter_kinds.len() != parameter_defaults.len()
            || !maximum_displacement.is_finite()
            || maximum_displacement < 0.0
            || parameter_defaults
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
        {
            return Err(VisualError::MalformedProgram);
        }
        super::validation::validate_instructions(
            &instructions,
            attributes.len(),
            &parameter_kinds,
        )?;
        let mut program_outputs = ProgramOutputs::default();
        for (output, register) in outputs {
            if program_outputs.get(*output).is_some() {
                return Err(VisualError::MalformedProgram);
            }
            let instruction = instructions
                .get(usize::from(*register))
                .ok_or(VisualError::MalformedProgram)?;
            if instruction.kind != super::validation::output_kind(*output) {
                return Err(VisualError::MalformedProgram);
            }
            if matches!(
                output,
                VisualOutput::RadiusScale | VisualOutput::WidthScale | VisualOutput::PositionOffset
            ) && instruction.stage == VisualStage::Fragment
            {
                return Err(VisualError::StageViolation { output: *output });
            }
            program_outputs.set(*output, *register);
        }
        if program_outputs.present == 0 {
            return Err(VisualError::MissingOutput);
        }
        let fingerprint = program_fingerprint(
            &instructions,
            &attributes,
            &parameter_kinds,
            &parameter_defaults,
            program_outputs,
            maximum_displacement,
        );
        Ok(Self {
            instructions: Arc::from(instructions),
            outputs: program_outputs,
            attributes: attributes.into(),
            properties: Arc::from(properties),
            parameter_kinds: Arc::from(parameter_kinds),
            parameter_defaults: Arc::from(parameter_defaults),
            parameter_owner: next_program_owner(),
            maximum_displacement,
            fingerprint,
        })
    }

    /// Number of instructions requiring per-fragment evaluation.
    #[must_use]
    pub fn fragment_instruction_count(&self) -> usize {
        self.stage_count(VisualStage::Fragment)
    }
    /// Number of instructions evaluated once per entity.
    #[must_use]
    pub fn entity_instruction_count(&self) -> usize {
        self.stage_count(VisualStage::Entity)
    }
    /// Number of instructions evaluated only when uniforms change.
    #[must_use]
    pub fn uniform_instruction_count(&self) -> usize {
        self.stage_count(VisualStage::Uniform)
    }

    fn stage_count(&self, stage: VisualStage) -> usize {
        self.instructions
            .iter()
            .filter(|instruction| instruction.stage == stage)
            .count()
    }

    /// Returns a typed handle for a scalar parameter slot.
    #[must_use]
    pub fn scalar_parameter(&self, index: usize) -> Option<ScalarParameter> {
        (self.parameter_kinds.get(index) == Some(&ValueKind::Scalar))
            .then(|| ScalarParameter(self.parameter(index)))
    }
    /// Returns a typed handle for a color parameter slot.
    #[must_use]
    pub fn color_parameter(&self, index: usize) -> Option<ColorParameter> {
        (self.parameter_kinds.get(index) == Some(&ValueKind::Color))
            .then(|| ColorParameter(self.parameter(index)))
    }
    /// Returns a typed handle for a vector parameter slot.
    #[must_use]
    pub fn vector_parameter(&self, index: usize) -> Option<VectorParameter> {
        (self.parameter_kinds.get(index) == Some(&ValueKind::Vector))
            .then(|| VectorParameter(self.parameter(index)))
    }
    /// A handle naming one parameter slot of this program.
    ///
    /// The parameter budget is far below `u8::MAX`, so an index that does not
    /// fit could not have come from a validated program; saturating keeps the
    /// accessor total and the resulting handle simply matches no slot.
    fn parameter(&self, index: usize) -> Parameter {
        Parameter {
            builder: self.parameter_owner,
            index: match u8::try_from(index) {
                Ok(index) => index,
                Err(_) => u8::MAX,
            },
        }
    }

    /// Checks this program against a drawable's supported channels.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::UnsupportedOutput`] naming the first channel the
    /// program writes that this drawable family cannot honour.
    pub fn validate_compatibility(
        &self,
        compatibility: VisualCompatibility,
    ) -> Result<(), VisualError> {
        for output in ALL_OUTPUTS {
            if self.outputs.get(output).is_some() && !compatibility.supports(output) {
                return Err(VisualError::UnsupportedOutput { output });
            }
        }
        Ok(())
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

/// One immutable program plus its compact mutable parameter block.
#[derive(Clone, PartialEq, Debug)]
pub struct VisualStyle {
    program: VisualProgram,
    parameters: Arc<[[f32; 4]]>,
}

impl VisualStyle {
    /// Uses a program's declared parameter defaults.
    #[must_use]
    pub fn new(program: VisualProgram) -> Self {
        Self {
            parameters: Arc::clone(&program.parameter_defaults),
            program,
        }
    }
    /// Underlying immutable program.
    #[must_use]
    pub const fn program(&self) -> &VisualProgram {
        &self.program
    }
    /// Current packed parameter values.
    #[must_use]
    pub fn parameters(&self) -> &[[f32; 4]] {
        &self.parameters
    }

    /// Deterministic revision key for allocation-free GPU parameter updates.
    #[must_use]
    pub fn parameter_fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for value in self.parameters.iter().flatten() {
            hash ^= u64::from(value.to_bits());
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    pub(crate) fn from_parameters(
        program: VisualProgram,
        parameters: Vec<[f32; 4]>,
    ) -> Result<Self, VisualError> {
        if parameters.len() != program.parameter_kinds.len()
            || parameters.iter().flatten().any(|value| !value.is_finite())
        {
            return Err(VisualError::InvalidParameter);
        }
        Ok(Self {
            program,
            parameters: Arc::from(parameters),
        })
    }
    /// Replaces one scalar parameter without changing the program.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the parameter belongs to a different
    /// program, was declared with a different type, or the value is not finite.
    pub fn set_scalar(
        &mut self,
        parameter: ScalarParameter,
        value: f32,
    ) -> Result<(), VisualError> {
        self.set_parameter(parameter.0, ValueKind::Scalar, [value, 0.0, 0.0, 0.0])
    }
    /// Replaces one linear-color parameter without changing the program.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the parameter belongs to a different
    /// program, was declared with a different type, or a component is not
    /// finite.
    pub fn set_color(
        &mut self,
        parameter: ColorParameter,
        value: [f32; 4],
    ) -> Result<(), VisualError> {
        self.set_parameter(parameter.0, ValueKind::Color, value)
    }
    /// Replaces one vector parameter without changing the program.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the parameter belongs to a different
    /// program, was declared with a different type, or a component is not
    /// finite.
    pub fn set_vector(
        &mut self,
        parameter: VectorParameter,
        value: [f32; 3],
    ) -> Result<(), VisualError> {
        self.set_parameter(
            parameter.0,
            ValueKind::Vector,
            [value[0], value[1], value[2], 0.0],
        )
    }
    fn set_parameter(
        &mut self,
        parameter: Parameter,
        kind: ValueKind,
        value: [f32; 4],
    ) -> Result<(), VisualError> {
        let index = usize::from(parameter.index);
        if parameter.builder != self.program.parameter_owner
            || self.program.parameter_kinds.get(index) != Some(&kind)
        {
            return Err(VisualError::InvalidParameter);
        }
        if value.iter().any(|component| !component.is_finite()) {
            return Err(VisualError::NonFinite);
        }
        Arc::make_mut(&mut self.parameters)[index] = value;
        Ok(())
    }
}

impl fmt::Display for VisualProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "visual-program:{:016x}", self.fingerprint)
    }
}
