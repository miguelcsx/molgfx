//! Straight-line WGSL emission for specialized visual programs.
//!
//! A validated [`VisualProgram`] is a fixed, bounded instruction list, so the
//! interpreter's loop over `visual_config.counts.x` with a dynamic opcode
//! dispatch can be replaced by one statement per instruction, with every
//! operand reference and every opcode resolved at generation time. This module
//! emits that body.
//!
//! # Why the emitted body is a strict drop-in
//!
//! The emitted `visual_resolve` keeps the interpreter's interface exactly: it
//! declares the same zero-initialized `array<vec4f, 64>` register file, writes
//! the live registers by static index, and then hands the file to the ladder
//! that both strategies share — `visual_resolve_registers`, which stays in WGSL
//! alone. Nothing about the output ladder, the uniform layout, or the
//! `counts.z` gate is duplicated here, so a specialized unit and an interpreted
//! unit resolve identical pixels from the same uniform.
//!
//! Because the register file keeps its full width and zero initialization, the
//! dynamic output-register lookups the ladder performs behave identically under
//! both strategies even if the bound uniform disagreed with the emitted
//! program. Specialization removes work; it does not add an assumption.
//!
//! # Supported instructions
//!
//! Every opcode except [`Opcode::Property`] and [`Opcode::State`], whose address
//! arithmetic depends on runtime column tables and the interaction arena rather
//! than on the instruction stream. A program containing either is reported as
//! [`VisualEmitError::UnsupportedOpcode`] and keeps the interpreter permanently.

use super::{Instruction, MAX_VISUAL_INSTRUCTIONS, Opcode, ValueKind, VisualProgram};
use std::fmt::Write as _;

/// Which evaluation strategy a style's draw uses.
///
/// The label is the single authoritative spelling of each state, so an API-side
/// and an engine-side report cannot drift apart. Only [`Self::Specialized`]
/// executes generated code; the other three all draw through the bounded typed
/// bytecode interpreter, and differ only in whether a specialized pipeline is
/// still coming, never will, or already arrived.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VisualPipeline {
    /// Straight-line code emitted from this exact program.
    Specialized,
    /// The interpreter, selected permanently because the program is not
    /// lowerable.
    Interpreter,
    /// The interpreter, until an in-flight compile lands.
    Pending,
    /// The interpreter, permanently, because specialization failed to compile.
    Failed,
}

impl VisualPipeline {
    /// Stable report label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Specialized => "specialized",
            Self::Interpreter => "typed-bytecode-interpreter",
            Self::Pending => "specialized-pending",
            Self::Failed => "failed",
        }
    }

    /// Whether the drawn pipeline is the interpreter.
    #[must_use]
    pub const fn is_interpreter(self) -> bool {
        !matches!(self, Self::Specialized)
    }

    /// The default reason for a state that needs no external diagnostic.
    #[must_use]
    pub const fn default_reason(self) -> &'static str {
        match self {
            Self::Specialized => "emitted-straight-line",
            Self::Interpreter => "program-not-lowerable",
            Self::Pending => "compiling",
            Self::Failed => "compile-failed",
        }
    }

    /// One report block naming the strategy, the cache key and the reason.
    ///
    /// The state and the reasons are passed in rather than carried here so a
    /// caller can report the specific instruction or compile diagnostic that
    /// produced the state.
    #[must_use]
    pub fn report(self, key: &str, reason: &str) -> String {
        format!(
            "pipeline: {}\ncache-key: {key}\nreason: {reason}",
            self.label()
        )
    }
}

/// The strategy a program selects before any compile has been attempted.
///
/// This is the eligibility half of the decision: a program either lowers
/// straight-line ([`VisualPipeline::Specialized`]) or never will
/// ([`VisualPipeline::Interpreter`], with the blocking instruction as the
/// reason). The runtime states — pending and failed — belong to the cache that
/// actually compiles the pipeline, which reports them alongside this.
#[must_use]
pub fn visual_pipeline(program: &VisualProgram) -> (VisualPipeline, String) {
    match unsupported(program) {
        None => (
            VisualPipeline::Specialized,
            VisualPipeline::Specialized.default_reason().to_owned(),
        ),
        Some(error) => (VisualPipeline::Interpreter, error.reason()),
    }
}

/// Why one program cannot become a specialized unit.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum VisualEmitError {
    /// One instruction uses an opcode the emitter does not lower.
    #[error("visual instruction {index} uses unsupported opcode {name} ({opcode})")]
    UnsupportedOpcode {
        /// Position of the instruction in the program.
        index: usize,
        /// Numeric opcode.
        opcode: u32,
        /// Stable opcode name.
        name: &'static str,
    },
    /// The program is larger than the portable bound.
    #[error("visual program with {count} instructions exceeds the portable bound")]
    InstructionLimit {
        /// Observed instruction count.
        count: usize,
    },
}

impl VisualEmitError {
    /// Short, stable reason suitable for an `explain` report.
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::UnsupportedOpcode { index, name, .. } => {
                format!("unsupported-opcode:{name}@{index}")
            }
            Self::InstructionLimit { count } => format!("instruction-limit:{count}"),
        }
    }
}

/// Stable name of a numeric opcode, for diagnostics.
#[must_use]
pub const fn opcode_name(opcode: u32) -> &'static str {
    match opcode {
        0 => "constant",
        1 => "input",
        2 => "property",
        3 => "parameter",
        4 => "add",
        5 => "subtract",
        6 => "multiply",
        7 => "safe-divide",
        8 => "abs",
        9 => "minimum",
        10 => "maximum",
        11 => "clamp",
        12 => "step",
        13 => "smooth-step",
        14 => "sine",
        15 => "mix",
        16 => "less",
        17 => "greater",
        18 => "and",
        19 => "or",
        20 => "not",
        21 => "select",
        22 => "dot",
        23 => "normalize",
        24 => "state",
        _ => "unknown",
    }
}

/// Whether one opcode can be lowered to straight-line WGSL.
#[must_use]
pub const fn is_supported(opcode: u32) -> bool {
    opcode <= Opcode::State as u32
        && opcode != Opcode::Property as u32
        && opcode != Opcode::State as u32
}

/// Whether every instruction of a program can be lowered.
///
/// A program this reports `false` for keeps the interpreter permanently; use
/// [`unsupported`] for the blocking instruction.
#[must_use]
pub fn is_specializable(program: &VisualProgram) -> bool {
    unsupported(program).is_none()
}

/// The first instruction that blocks specialization, when one exists.
#[must_use]
pub fn unsupported(program: &VisualProgram) -> Option<VisualEmitError> {
    if program.instructions.len() > MAX_VISUAL_INSTRUCTIONS {
        return Some(VisualEmitError::InstructionLimit {
            count: program.instructions.len(),
        });
    }
    program
        .instructions
        .iter()
        .enumerate()
        .find(|(_, instruction)| !is_supported(instruction.opcode()))
        .map(|(index, instruction)| VisualEmitError::UnsupportedOpcode {
            index,
            opcode: instruction.opcode(),
            name: opcode_name(instruction.opcode()),
        })
}

/// Emits the straight-line `visual_resolve` body for one program.
///
/// The returned text replaces the `// {{visual_program}}` marker in the
/// specialized sibling of a composed unit, and is only valid for the program it
/// was emitted from.
///
/// # Errors
///
/// Returns the blocking instruction when the program uses an unsupported opcode
/// or exceeds the instruction bound.
pub fn emit_resolve(program: &VisualProgram) -> Result<String, VisualEmitError> {
    let mut source = String::with_capacity(128 + program.instructions.len() * 112);
    source.push_str(LADDER_HEAD);
    for (index, instruction) in program.instructions.iter().enumerate() {
        emit_instruction(&mut source, index, instruction)?;
    }
    source.push_str(LADDER_TAIL);
    Ok(source)
}

/// The gate and register declaration that open every emitted body.
const LADDER_HEAD: &str = "\
fn visual_resolve(
    inputs: VisualEvaluationInputs,
    fallback: VisualFragmentResult,
) -> VisualFragmentResult {
    if !VISUAL_FRAGMENT_ENABLED || visual_config.counts.z == 0u {
        return fallback;
    }
    var registers: array<vec4f, 64>;
";

/// The hand-off to the shared ladder that closes every emitted body.
const LADDER_TAIL: &str = "    return visual_resolve_registers(&registers, fallback);\n}\n";

fn emit_instruction(
    source: &mut String,
    index: usize,
    instruction: &Instruction,
) -> Result<(), VisualEmitError> {
    let opcode = instruction.opcode();
    if !is_supported(opcode) {
        // Rechecked rather than assumed: this is the emitter's own correctness
        // boundary, and an unsupported opcode must never silently emit a
        // zero-valued register in place of real work.
        return Err(VisualEmitError::UnsupportedOpcode {
            index,
            opcode,
            name: opcode_name(opcode),
        });
    }
    let operands = instruction.operands();
    let operand = |position: usize| format!("registers[{}]", operands[position]);
    let (left, right) = (operand(0), operand(1));
    let third = operand(2);
    let value = match opcode {
        // The raw payload, exactly as the interpreter returns it before any
        // kind-specific interpretation.
        0 => {
            let data = instruction.data();
            format!(
                "vec4f({}, {}, {}, {})",
                float(data[0]),
                float(data[1]),
                float(data[2]),
                float(data[3])
            )
        }
        1 => format!("visual_input({}u, inputs)", slot(instruction)),
        3 => format!("visual_parameter({}u)", slot(instruction)),
        4..=6 => componentwise(instruction, &left, &right, opcode),
        7 => format!(
            "vec4f(select({left}.x / {right}.x, 0.0, abs({right}.x) <= 1e-8), 0.0, 0.0, 0.0)"
        ),
        8 => format!("vec4f(abs({left}.x), 0.0, 0.0, 0.0)"),
        9 => format!("vec4f(visual_ordered_minimum({left}.x, {right}.x), 0.0, 0.0, 0.0)"),
        10 => format!("vec4f(visual_ordered_maximum({left}.x, {right}.x), 0.0, 0.0, 0.0)"),
        11 => format!("vec4f(visual_ordered_clamp({left}.x, {right}.x, {third}.x), 0.0, 0.0, 0.0)"),
        12 => format!("vec4f(visual_ordered_step({left}.x, {right}.x), 0.0, 0.0, 0.0)"),
        13 => format!(
            concat!(
                "vec4f(select(0.0, smoothstep({left}.x, {right}.x, {third}.x), ",
                "{left}.x == {left}.x && {right}.x == {right}.x && {left}.x < {right}.x), ",
                "0.0, 0.0, 0.0)"
            ),
            left = left,
            right = right,
            third = third
        ),
        14 => format!("vec4f(sin({left}.x), 0.0, 0.0, 0.0)"),
        15 => format!("mix({left}, {right}, vec4f({third}.x))"),
        16 => format!("vec4f(select(0.0, 1.0, {left}.x < {right}.x), 0.0, 0.0, 0.0)"),
        17 => format!("vec4f(select(0.0, 1.0, {left}.x > {right}.x), 0.0, 0.0, 0.0)"),
        18 => format!(
            "vec4f(select(0.0, 1.0, visual_truth({left}) && visual_truth({right})), 0.0, 0.0, 0.0)"
        ),
        19 => format!(
            "vec4f(select(0.0, 1.0, visual_truth({left}) || visual_truth({right})), 0.0, 0.0, 0.0)"
        ),
        20 => format!("vec4f(select(0.0, 1.0, !visual_truth({left})), 0.0, 0.0, 0.0)"),
        21 => format!("select({third}, {right}, visual_truth({left}))"),
        22 => format!("vec4f(dot({left}.xyz, {right}.xyz), 0.0, 0.0, 0.0)"),
        23 => {
            // The squared length is bound once rather than repeated, because a
            // single straight-line expression would evaluate the dot product
            // four times.
            let _ = writeln!(
                source,
                "    let length_squared_{index} = dot({left}.xyz, {left}.xyz);"
            );
            format!(
                concat!(
                    "vec4f(select({left}.xyz * inverseSqrt(length_squared_{index}), ",
                    "vec3f(0.0), length_squared_{index} <= 1e-16 ",
                    "|| length_squared_{index} != length_squared_{index} ",
                    "|| abs(length_squared_{index}) > 3.402823466e+38), 0.0)"
                ),
                left = left,
                index = index
            )
        }
        // Every opcode is matched above; the guard rejects the two the emitter
        // does not lower, so this arm is unreachable for a validated program.
        _ => {
            return Err(VisualEmitError::UnsupportedOpcode {
                index,
                opcode,
                name: opcode_name(opcode),
            });
        }
    };
    let _ = writeln!(source, "    registers[{index}] = {value};");
    Ok(())
}

/// The interpreter's component-wise arithmetic, resolved for a static kind.
fn componentwise(instruction: &Instruction, left: &str, right: &str, opcode: u32) -> String {
    let operator = match opcode {
        4 => "+",
        5 => "-",
        _ => "*",
    };
    match instruction.kind() {
        // The interpreter's component count is one for a scalar, so only `.x`
        // carries a value.
        ValueKind::Scalar => format!("vec4f({left}.x {operator} {right}.x, 0.0, 0.0, 0.0)"),
        // Color and boolean kinds fall through to the interpreter's own
        // component-wise helper, which owns their component count.
        ValueKind::Color | ValueKind::Bool => {
            format!(
                "visual_componentwise({}u, {left}, {right}, {opcode}u)",
                instruction.kind() as u32
            )
        }
        // A vector multiplied by a scalar is the interpreter's one special
        // case that is not component-wise.
        ValueKind::Vector if opcode == 6 => format!("vec4f({left}.xyz * {right}.x, 0.0)"),
        ValueKind::Vector => format!(
            concat!(
                "vec4f({left}.x {operator} {right}.x, {left}.y {operator} {right}.y, ",
                "{left}.z {operator} {right}.z, 0.0)"
            ),
            left = left,
            right = right,
            operator = operator
        ),
    }
}

/// Slot operand of a metadata-carrying instruction.
fn slot(instruction: &Instruction) -> usize {
    super::numeric::decode_slot(instruction.data()[0])
}

/// A WGSL float literal, which must carry an exponent or a decimal point so the
/// parser never reads it as an integer literal.
fn float(value: f32) -> String {
    format!("{value:e}")
}
