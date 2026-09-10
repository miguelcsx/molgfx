//! Immutable visual-program values and limits.

use crate::RepresentationKind;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROGRAM_OWNER: AtomicU64 = AtomicU64::new(1);

pub(super) fn next_program_owner() -> u64 {
    NEXT_PROGRAM_OWNER.fetch_add(1, Ordering::Relaxed)
}

/// Maximum live instructions in one visual program.
pub const MAX_VISUAL_INSTRUCTIONS: usize = 64;
/// Maximum atom-property columns referenced by one visual program.
pub const MAX_VISUAL_PROPERTIES: usize = 4;
/// Maximum dynamic parameters in one visual program.
pub const MAX_VISUAL_PARAMETERS: usize = 16;

/// A graph-local scalar expression.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalarExpr(pub(crate) Expr);
/// A graph-local linear-color expression.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ColorExpr(pub(crate) Expr);
/// A graph-local boolean expression.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BoolExpr(pub(crate) Expr);
/// A graph-local three-component vector expression.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VectorExpr(pub(crate) Expr);

/// A scalar parameter slot declared by a visual program.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalarParameter(pub(crate) Parameter);
/// A linear-color parameter slot declared by a visual program.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ColorParameter(pub(crate) Parameter);
/// A three-component vector parameter slot declared by a visual program.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VectorParameter(pub(crate) Parameter);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Expr {
    pub(crate) builder: u64,
    pub(crate) register: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Parameter {
    pub(crate) builder: u64,
    pub(crate) index: u8,
}

/// A visual channel a program may write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum VisualOutput {
    /// Linear RGBA surface color.
    BaseColor,
    /// Coverage/optical opacity.
    Opacity,
    /// Additive linear HDR emission.
    Emission,
    /// Perceptual micro-surface roughness.
    Roughness,
    /// Dielectric/specular response strength.
    Specular,
    /// Scalar parameter of the selected material model.
    MaterialStrength,
    /// Fragment and entity visibility.
    Visibility,
    /// Analytic silhouette transition in physical pixels.
    SilhouetteSoftness,
    /// Analytic radius multiplier.
    RadiusScale,
    /// Ribbon, line or capsule width multiplier.
    WidthScale,
    /// Bounded local presentation displacement.
    PositionOffset,
}

impl VisualOutput {
    pub(crate) const fn code(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_code(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::BaseColor),
            1 => Some(Self::Opacity),
            2 => Some(Self::Emission),
            3 => Some(Self::Roughness),
            4 => Some(Self::Specular),
            5 => Some(Self::MaterialStrength),
            6 => Some(Self::Visibility),
            7 => Some(Self::SilhouetteSoftness),
            8 => Some(Self::RadiusScale),
            9 => Some(Self::WidthScale),
            10 => Some(Self::PositionOffset),
            _ => None,
        }
    }
}

/// Supported outputs for one drawable family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VisualCompatibility(u16);

impl VisualCompatibility {
    /// No visual-program channels. Used until a drawable family has a native
    /// lowering rather than accepting a style it cannot honour.
    pub const NONE: Self = Self(0);
    /// Appearance channels shared by opaque and transparent shaded drawables.
    pub const APPEARANCE: Self = Self(bits(&[
        VisualOutput::BaseColor,
        VisualOutput::Opacity,
        VisualOutput::Emission,
        VisualOutput::Roughness,
        VisualOutput::Specular,
        VisualOutput::MaterialStrength,
        VisualOutput::Visibility,
    ]));
    /// Shaded appearance plus analytic silhouette control.
    pub const ANALYTIC_APPEARANCE: Self =
        Self(Self::APPEARANCE.0 | bit(VisualOutput::SilhouetteSoftness));
    /// Appearance plus analytic radius/width scaling.
    pub const SIZED: Self = Self(
        Self::ANALYTIC_APPEARANCE.0
            | bit(VisualOutput::RadiusScale)
            | bit(VisualOutput::WidthScale),
    );
    /// Appearance, size and bounded local displacement.
    pub const DEFORMABLE: Self = Self(Self::SIZED.0 | bit(VisualOutput::PositionOffset));
    /// Shaded ribbon channels plus bounded local displacement. Ribbon width
    /// remains fixed until its centreline-relative scale has native lowering.
    pub const RIBBON: Self = Self(Self::APPEARANCE.0 | bit(VisualOutput::PositionOffset));
    /// Generic points: analytic appearance, radius and bounded displacement.
    pub const POINTS: Self = Self(
        Self::ANALYTIC_APPEARANCE.0
            | bit(VisualOutput::RadiusScale)
            | bit(VisualOutput::PositionOffset),
    );
    /// Shared analytic template instances and their local parts.
    pub const INSTANCES: Self = Self(
        Self::ANALYTIC_APPEARANCE.0
            | bit(VisualOutput::RadiusScale)
            | bit(VisualOutput::PositionOffset),
    );
    /// Generic analytic relations: unlit color/emission, coverage, visibility
    /// and physical-pixel width. Surface-response channels are intentionally
    /// absent because a screen-space line has no physical shading frame.
    pub const RELATIONS: Self = Self(bits(&[
        VisualOutput::BaseColor,
        VisualOutput::Opacity,
        VisualOutput::Emission,
        VisualOutput::Visibility,
        VisualOutput::WidthScale,
    ]));

    /// Returns whether this family accepts an output channel.
    #[must_use]
    pub const fn supports(self, output: VisualOutput) -> bool {
        self.0 & bit(output) != 0
    }

    /// Compatibility for one built-in representation kind.
    #[must_use]
    pub const fn for_representation(kind: RepresentationKind) -> Self {
        match kind {
            RepresentationKind::Surface => Self::APPEARANCE,
            RepresentationKind::Spacefill
            | RepresentationKind::BallAndStick
            | RepresentationKind::Licorice
            | RepresentationKind::Lines
            | RepresentationKind::Beads
            | RepresentationKind::Points => Self::DEFORMABLE,
            RepresentationKind::Cartoon
            | RepresentationKind::Trace
            | RepresentationKind::Tube
            | RepresentationKind::Rocket
            | RepresentationKind::Twister
            | RepresentationKind::PaperChain => Self::RIBBON,
            RepresentationKind::Volume | RepresentationKind::Segmentation => Self::NONE,
        }
    }
}

const fn bit(output: VisualOutput) -> u16 {
    1u16 << output as u16
}

const fn bits(outputs: &[VisualOutput]) -> u16 {
    let mut result = 0;
    let mut index = 0;
    while index < outputs.len() {
        result |= bit(outputs[index]);
        index += 1;
    }
    result
}

/// Visual-program construction or compatibility failure.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum VisualError {
    /// A node from another builder was used.
    #[error("expression belongs to another visual-program builder")]
    ForeignExpression,
    /// More instructions were requested than the portable bound.
    #[error("visual program exceeds its 64-instruction limit")]
    InstructionLimit,
    /// More property columns were requested than the portable bound.
    #[error("visual program references more than four atom properties")]
    PropertyLimit,
    /// More dynamic parameters were requested than the portable bound.
    #[error("visual program declares more than sixteen parameters")]
    ParameterLimit,
    /// A literal or parameter contained NaN or infinity.
    #[error("visual constants and parameter values must be finite")]
    NonFinite,
    /// A declared interval or displacement bound is malformed.
    #[error("visual range must be finite and strictly increasing")]
    InvalidRange,
    /// The program was frozen without any observable output.
    #[error("visual program must write at least one output")]
    MissingOutput,
    /// An output channel was assigned more than once.
    #[error("visual output {output:?} is already assigned")]
    DuplicateOutput {
        /// The channel that already has a register.
        output: VisualOutput,
    },
    /// The target drawable cannot consume one requested channel.
    #[error("visual output {output:?} is unsupported by this drawable")]
    UnsupportedOutput {
        /// Unsupported channel.
        output: VisualOutput,
    },
    /// A parameter handle came from another program or has another type.
    #[error("visual parameter does not belong to this program")]
    InvalidParameter,
    /// A scene-independent program and its residency-ticket bindings disagree.
    #[error("paged visual columns must be bound exactly once with matching keys")]
    InvalidColumnBinding,
    /// Geometry-stage output depends on fragment-only information.
    #[error("visual output {output:?} depends on fragment-only inputs")]
    StageViolation {
        /// Geometry-stage channel with an invalid dependency.
        output: VisualOutput,
    },
    /// A serialized instruction stream violates the typed graph invariants.
    #[error("visual program instruction stream is malformed")]
    MalformedProgram,
}

/// Earliest GPU stage at which an instruction can be evaluated.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum VisualStage {
    /// Once when parameters change.
    Uniform,
    /// Once per entity in the GPU preparation/compaction pass.
    Entity,
    /// Once per shaded fragment only when view-dependent data is required.
    Fragment,
}

impl VisualStage {
    pub(crate) const fn code(self) -> u8 {
        self as u8
    }

    pub(crate) const fn from_code(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Uniform),
            1 => Some(Self::Entity),
            2 => Some(Self::Fragment),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ValueKind {
    Scalar,
    Color,
    Bool,
    Vector,
}

impl ValueKind {
    pub(crate) const fn code(self) -> u8 {
        self as u8
    }

    pub(crate) const fn from_code(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Scalar),
            1 => Some(Self::Color),
            2 => Some(Self::Bool),
            3 => Some(Self::Vector),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub(crate) enum Opcode {
    Constant = 0,
    Input = 1,
    Property = 2,
    Parameter = 3,
    Add = 4,
    Subtract = 5,
    Multiply = 6,
    SafeDivide = 7,
    Abs = 8,
    Minimum = 9,
    Maximum = 10,
    Clamp = 11,
    Step = 12,
    SmoothStep = 13,
    Sine = 14,
    Mix = 15,
    Less = 16,
    Greater = 17,
    And = 18,
    Or = 19,
    Not = 20,
    Select = 21,
    Dot = 22,
    Normalize = 23,
}

impl Opcode {
    pub(crate) const fn from_code(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Constant),
            1 => Some(Self::Input),
            2 => Some(Self::Property),
            3 => Some(Self::Parameter),
            4 => Some(Self::Add),
            5 => Some(Self::Subtract),
            6 => Some(Self::Multiply),
            7 => Some(Self::SafeDivide),
            8 => Some(Self::Abs),
            9 => Some(Self::Minimum),
            10 => Some(Self::Maximum),
            11 => Some(Self::Clamp),
            12 => Some(Self::Step),
            13 => Some(Self::SmoothStep),
            14 => Some(Self::Sine),
            15 => Some(Self::Mix),
            16 => Some(Self::Less),
            17 => Some(Self::Greater),
            18 => Some(Self::And),
            19 => Some(Self::Or),
            20 => Some(Self::Not),
            21 => Some(Self::Select),
            22 => Some(Self::Dot),
            23 => Some(Self::Normalize),
            _ => None,
        }
    }

    pub(crate) const fn operand_count(self) -> usize {
        match self {
            Self::Abs | Self::Sine | Self::Not | Self::Normalize => 1,
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::SafeDivide
            | Self::Minimum
            | Self::Maximum
            | Self::Step
            | Self::Less
            | Self::Greater
            | Self::And
            | Self::Or
            | Self::Dot => 2,
            Self::Clamp | Self::SmoothStep | Self::Mix | Self::Select => 3,
            Self::Constant | Self::Input | Self::Property | Self::Parameter => 0,
        }
    }
}

/// One fixed-width instruction consumed by the CPU and GPU evaluators.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Instruction {
    pub(crate) opcode: Opcode,
    pub(crate) kind: ValueKind,
    pub(crate) operands: [u8; 3],
    pub(crate) data: [f32; 4],
    pub(crate) stage: VisualStage,
}

impl Instruction {
    /// Stable numeric opcode used by the GPU instruction table.
    #[must_use]
    pub const fn opcode(&self) -> u32 {
        self.opcode as u32
    }

    /// Source register indices.
    #[must_use]
    pub const fn operands(&self) -> [u8; 3] {
        self.operands
    }

    /// Literal payload or instruction metadata.
    #[must_use]
    pub const fn data(&self) -> [f32; 4] {
        self.data
    }

    /// Earliest stage at which this instruction can execute.
    #[must_use]
    pub const fn stage(&self) -> VisualStage {
        self.stage
    }

    pub(crate) const fn kind_code(&self) -> u8 {
        self.kind.code()
    }

    pub(crate) fn from_serialized(
        opcode: u32,
        kind: u8,
        operands: [u8; 3],
        data: [f32; 4],
        stage: u8,
    ) -> Result<Self, VisualError> {
        if data.iter().any(|value| !value.is_finite()) {
            return Err(VisualError::NonFinite);
        }
        Ok(Self {
            opcode: Opcode::from_code(opcode).ok_or(VisualError::MalformedProgram)?,
            kind: ValueKind::from_code(kind).ok_or(VisualError::MalformedProgram)?,
            operands,
            data,
            stage: VisualStage::from_code(stage).ok_or(VisualError::MalformedProgram)?,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ProgramOutputs {
    pub(crate) registers: [u8; 11],
    pub(crate) present: u16,
}

impl ProgramOutputs {
    pub(crate) const fn get(self, output: VisualOutput) -> Option<u8> {
        if self.present & bit(output) == 0 {
            None
        } else {
            Some(self.registers[output as usize])
        }
    }

    pub(crate) fn set(&mut self, output: VisualOutput, register: u8) {
        self.registers[output as usize] = register;
        self.present |= bit(output);
    }
}
