//! Typed construction of bounded visual programs.

#[path = "builder/arithmetic.rs"]
mod arithmetic;
#[path = "builder/inputs.rs"]
mod inputs;
#[path = "builder/literals.rs"]
mod literals;
#[path = "builder/logic.rs"]
mod logic;
#[path = "builder/parameters.rs"]
mod parameters;
#[path = "builder/properties.rs"]
mod properties;
#[path = "builder/ramps.rs"]
mod ramps;

use super::types::next_program_owner;
use super::{Instruction, ProgramOutputs, ValueKind};
use crate::{AtomPropertyHandle, VisualAttributeRef};

/// Typed builder for a portable visual program.
#[derive(Debug)]
pub struct VisualProgramBuilder {
    pub(super) id: u64,
    pub(super) instructions: Vec<Instruction>,
    pub(super) attributes: Vec<VisualAttributeRef>,
    pub(super) properties: Vec<AtomPropertyHandle>,
    pub(super) parameter_kinds: Vec<ValueKind>,
    pub(super) parameter_defaults: Vec<[f32; 4]>,
    pub(super) outputs: ProgramOutputs,
    pub(super) maximum_displacement: f32,
}

impl Default for VisualProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualProgramBuilder {
    /// Starts an empty visual program.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: next_program_owner(),
            instructions: Vec::new(),
            attributes: Vec::new(),
            properties: Vec::new(),
            parameter_kinds: Vec::new(),
            parameter_defaults: Vec::new(),
            outputs: ProgramOutputs::default(),
            maximum_displacement: 0.0,
        }
    }
}
