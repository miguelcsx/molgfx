//! Safe declarative visual programs and their bounded evaluation contract.

mod builder;
mod builder_outputs;
mod compiler;
mod emit;
mod evaluate;
mod fingerprint;
mod numeric;
mod program;
mod recipes;
mod types;
mod validation;

pub use builder::VisualProgramBuilder;
pub use emit::{
    VisualEmitError, VisualPipeline, emit_resolve, is_specializable, is_supported, opcode_name,
    unsupported, visual_pipeline,
};
pub use evaluate::{VisualEvaluation, VisualInputs};
pub use program::{
    VisualAttributeRef, VisualColumnKey, VisualInstructionGpu, VisualProgram, VisualStyle,
};
pub use types::{
    BoolExpr, ColorExpr, ColorParameter, MAX_VISUAL_INSTRUCTIONS, MAX_VISUAL_PARAMETERS,
    MAX_VISUAL_PROPERTIES, ScalarExpr, ScalarParameter, VectorExpr, VectorParameter,
    VisualCompatibility, VisualError, VisualOutput, VisualStage,
};

pub(crate) use types::{Expr, Instruction, Opcode, Parameter, ProgramOutputs, ValueKind};

#[cfg(test)]
#[path = "emit_tests.rs"]
mod emit_tests;

#[cfg(test)]
mod tests;
