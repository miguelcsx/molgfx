//! Deterministic CPU reference evaluation for visual programs.

use super::numeric::{decode_code, decode_slot, entity_scalar};
use super::{MAX_VISUAL_INSTRUCTIONS, Opcode, ValueKind, VisualOutput, VisualProgram, VisualStyle};

/// Fully resolved inputs for one visual-program evaluation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VisualInputs {
    /// Built-in or previously resolved linear color.
    pub base_color: [f32; 4],
    /// Built-in material opacity.
    pub base_opacity: f32,
    /// Current presentation time in seconds.
    pub time_seconds: f32,
    /// Shaded local-space position.
    pub local_position: [f32; 3],
    /// Shaded world-space position.
    pub world_position: [f32; 3],
    /// World-space normal.
    pub normal: [f32; 3],
    /// Direction from the point toward the camera.
    pub view_direction: [f32; 3],
    /// Distance to the camera in world units.
    pub camera_distance: f32,
    /// Stable entity row.
    pub entity_index: u32,
    /// Built-in perceptual roughness.
    pub roughness: f32,
    /// Built-in specular strength.
    pub specular: f32,
    /// Built-in model-specific material strength.
    pub material_strength: f32,
    /// Packed semantic interaction channels for this entity.
    pub interaction_bits: u32,
    /// Up to four generic typed attribute values, expanded to four lanes.
    pub attributes: [[f32; 4]; 4],
    /// Transitional scalar values for old programs.
    #[doc(hidden)]
    pub properties: [f32; 4],
}

impl Default for VisualInputs {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            base_opacity: 1.0,
            time_seconds: 0.0,
            local_position: [0.0; 3],
            world_position: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
            view_direction: [0.0, 0.0, 1.0],
            camera_distance: 0.0,
            entity_index: 0,
            roughness: 0.34,
            specular: 0.5,
            material_strength: 0.0,
            interaction_bits: 0,
            attributes: [[f32::NAN; 4]; 4],
            properties: [f32::NAN; 4],
        }
    }
}

/// Final bounded visual values after program evaluation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VisualEvaluation {
    /// Final linear surface color.
    pub base_color: [f32; 4],
    /// Final bounded opacity.
    pub opacity: f32,
    /// Final bounded HDR emission.
    pub emission: [f32; 3],
    /// Final bounded roughness.
    pub roughness: f32,
    /// Final bounded specular strength.
    pub specular: f32,
    /// Final bounded model-specific strength.
    pub material_strength: f32,
    /// Final visibility.
    pub visible: bool,
    /// Final silhouette softness in physical pixels.
    pub silhouette_softness: f32,
    /// Final analytic radius multiplier.
    pub radius_scale: f32,
    /// Final line/ribbon width multiplier.
    pub width_scale: f32,
    /// Final bounded local displacement.
    pub position_offset: [f32; 3],
}

impl VisualProgram {
    /// Evaluates with declared parameter defaults.
    #[must_use]
    pub fn evaluate(&self, inputs: VisualInputs) -> VisualEvaluation {
        evaluate(self, &self.parameter_defaults, inputs)
    }
}

impl VisualStyle {
    /// Evaluates with this style's current parameter values.
    #[must_use]
    pub fn evaluate(&self, inputs: VisualInputs) -> VisualEvaluation {
        evaluate(self.program(), self.parameters(), inputs)
    }
}

fn evaluate(
    program: &VisualProgram,
    parameters: &[[f32; 4]],
    inputs: VisualInputs,
) -> VisualEvaluation {
    let mut registers = [[0.0; 4]; MAX_VISUAL_INSTRUCTIONS];
    for (index, instruction) in program.instructions.iter().enumerate() {
        let [a, b, c] = instruction.operands;
        let left = registers[usize::from(a)];
        let right = registers[usize::from(b)];
        let third = registers[usize::from(c)];
        registers[index] = match instruction.opcode {
            Opcode::Constant => instruction.data,
            Opcode::Input => input_value(decode_code(instruction.data[0]), inputs),
            Opcode::Property => {
                let slot = decode_slot(instruction.data[0]);
                if instruction.data[1] == 0.0 {
                    let value = match inputs.properties.get(slot) {
                        Some(value) => *value,
                        None => f32::NAN,
                    };
                    [value, 0.0, 0.0, 0.0]
                } else {
                    match inputs.attributes.get(slot) {
                        Some(value) => *value,
                        None => [f32::NAN; 4],
                    }
                }
            }
            Opcode::Parameter => match parameters.get(decode_slot(instruction.data[0])) {
                Some(value) => *value,
                None => [0.0; 4],
            },
            Opcode::Add => componentwise(instruction.kind, left, right, |x, y| x + y),
            Opcode::Subtract => componentwise(instruction.kind, left, right, |x, y| x - y),
            Opcode::Multiply => {
                if instruction.kind == ValueKind::Vector {
                    [
                        left[0] * right[0],
                        left[1] * right[0],
                        left[2] * right[0],
                        0.0,
                    ]
                } else {
                    componentwise(instruction.kind, left, right, |x, y| x * y)
                }
            }
            Opcode::SafeDivide => [safe_divide(left[0], right[0]), 0.0, 0.0, 0.0],
            Opcode::Abs => [left[0].abs(), 0.0, 0.0, 0.0],
            Opcode::Minimum => [ordered_minimum(left[0], right[0]), 0.0, 0.0, 0.0],
            Opcode::Maximum => [ordered_maximum(left[0], right[0]), 0.0, 0.0, 0.0],
            Opcode::Clamp => [ordered_clamp(left[0], right[0], third[0]), 0.0, 0.0, 0.0],
            Opcode::Step => [ordered_step(left[0], right[0]), 0.0, 0.0, 0.0],
            Opcode::SmoothStep => [smoothstep(left[0], right[0], third[0]), 0.0, 0.0, 0.0],
            Opcode::Sine => [left[0].sin(), 0.0, 0.0, 0.0],
            Opcode::Mix => mix(instruction.kind, left, right, third[0]),
            Opcode::Less => [f32::from(u8::from(left[0] < right[0])), 0.0, 0.0, 0.0],
            Opcode::Greater => [f32::from(u8::from(left[0] > right[0])), 0.0, 0.0, 0.0],
            Opcode::And => [
                f32::from(u8::from(truth(left) && truth(right))),
                0.0,
                0.0,
                0.0,
            ],
            Opcode::Or => [
                f32::from(u8::from(truth(left) || truth(right))),
                0.0,
                0.0,
                0.0,
            ],
            Opcode::Not => [f32::from(u8::from(!truth(left))), 0.0, 0.0, 0.0],
            Opcode::Select => {
                if truth(left) {
                    right
                } else {
                    third
                }
            }
            Opcode::Dot => [
                left[0] * right[0] + left[1] * right[1] + left[2] * right[2],
                0.0,
                0.0,
                0.0,
            ],
            Opcode::Normalize => normalize(left),
            Opcode::State => [
                f32::from(u8::from(
                    inputs.interaction_bits & instruction.data[0].to_bits() != 0,
                )),
                0.0,
                0.0,
                0.0,
            ],
        };
    }
    resolve(program, &registers, inputs)
}

fn resolve(
    program: &VisualProgram,
    registers: &[[f32; 4]; MAX_VISUAL_INSTRUCTIONS],
    inputs: VisualInputs,
) -> VisualEvaluation {
    let color = match output(program, registers, VisualOutput::BaseColor) {
        Some(value) => value,
        None => inputs.base_color,
    };
    let emission = output(program, registers, VisualOutput::Emission)
        .into_iter()
        .fold([0.0; 4], |_, value| value);
    let offset = output(program, registers, VisualOutput::PositionOffset)
        .into_iter()
        .fold([0.0; 4], |_, value| value);
    VisualEvaluation {
        base_color: finite_or(color, inputs.base_color).map(|value| value.clamp(0.0, 1.0)),
        opacity: scalar_output(
            program,
            registers,
            VisualOutput::Opacity,
            inputs.base_opacity,
        )
        .clamp(0.0, 1.0),
        emission: [
            finite_component(emission[0], 0.0).clamp(0.0, 64.0),
            finite_component(emission[1], 0.0).clamp(0.0, 64.0),
            finite_component(emission[2], 0.0).clamp(0.0, 64.0),
        ],
        roughness: scalar_output(
            program,
            registers,
            VisualOutput::Roughness,
            inputs.roughness,
        )
        .clamp(0.05, 0.92),
        specular: scalar_output(program, registers, VisualOutput::Specular, inputs.specular)
            .clamp(0.0, 1.0),
        material_strength: scalar_output(
            program,
            registers,
            VisualOutput::MaterialStrength,
            inputs.material_strength,
        )
        .clamp(0.0, 1.0),
        visible: output(program, registers, VisualOutput::Visibility).is_none_or(truth),
        silhouette_softness: scalar_output(
            program,
            registers,
            VisualOutput::SilhouetteSoftness,
            0.0,
        )
        .clamp(0.0, 8.0),
        radius_scale: scalar_output(program, registers, VisualOutput::RadiusScale, 1.0)
            .clamp(0.0, 4.0),
        width_scale: scalar_output(program, registers, VisualOutput::WidthScale, 1.0)
            .clamp(0.0, 4.0),
        position_offset: bounded_offset(offset, program.maximum_displacement),
    }
}

fn input_value(input: u8, values: VisualInputs) -> [f32; 4] {
    match input {
        0 => values.base_color,
        1 => [values.base_opacity, 0.0, 0.0, 0.0],
        2 => [values.time_seconds, 0.0, 0.0, 0.0],
        3 => vector(values.local_position),
        4 => vector(values.world_position),
        5 => vector(values.normal),
        6 => vector(values.view_direction),
        7 => [values.camera_distance, 0.0, 0.0, 0.0],
        8 => [entity_scalar(values.entity_index), 0.0, 0.0, 0.0],
        9 => [values.roughness, 0.0, 0.0, 0.0],
        10 => [values.specular, 0.0, 0.0, 0.0],
        11 => [values.material_strength, 0.0, 0.0, 0.0],
        _ => [0.0; 4],
    }
}

fn vector(value: [f32; 3]) -> [f32; 4] {
    [value[0], value[1], value[2], 0.0]
}

fn componentwise(
    kind: ValueKind,
    left: [f32; 4],
    right: [f32; 4],
    operation: impl Fn(f32, f32) -> f32,
) -> [f32; 4] {
    let count = match kind {
        ValueKind::Scalar | ValueKind::Bool => 1,
        ValueKind::Vector => 3,
        ValueKind::Color => 4,
    };
    let mut output = [0.0; 4];
    for index in 0..count {
        output[index] = operation(left[index], right[index]);
    }
    output
}

fn mix(kind: ValueKind, from: [f32; 4], to: [f32; 4], weight: f32) -> [f32; 4] {
    componentwise(kind, from, to, |left, right| left + (right - left) * weight)
}

fn safe_divide(numerator: f32, denominator: f32) -> f32 {
    if denominator.abs() <= 1.0e-8 {
        0.0
    } else {
        numerator / denominator
    }
}

fn ordered_minimum(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else {
        left.min(right)
    }
}

fn ordered_maximum(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else {
        left.max(right)
    }
}

fn ordered_clamp(value: f32, low: f32, high: f32) -> f32 {
    if !value.is_finite() || !low.is_finite() || !high.is_finite() || low > high {
        f32::NAN
    } else {
        value.max(low).min(high)
    }
}

fn ordered_step(edge: f32, value: f32) -> f32 {
    f32::from(u8::from(
        edge.is_finite() && value.is_finite() && value >= edge,
    ))
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    if !low.is_finite() || !high.is_finite() || low >= high {
        return 0.0;
    }
    let parameter = ((value - low) / (high - low)).clamp(0.0, 1.0);
    parameter * parameter * (3.0 - 2.0 * parameter)
}

fn normalize(value: [f32; 4]) -> [f32; 4] {
    let length_squared = value[0] * value[0] + value[1] * value[1] + value[2] * value[2];
    if length_squared <= 1.0e-16 || !length_squared.is_finite() {
        return [0.0; 4];
    }
    let inverse = length_squared.sqrt().recip();
    [
        value[0] * inverse,
        value[1] * inverse,
        value[2] * inverse,
        0.0,
    ]
}

fn truth(value: [f32; 4]) -> bool {
    value[0].is_finite() && value[0] != 0.0
}

fn output(
    program: &VisualProgram,
    registers: &[[f32; 4]; MAX_VISUAL_INSTRUCTIONS],
    output: VisualOutput,
) -> Option<[f32; 4]> {
    program
        .outputs
        .get(output)
        .map(|register| registers[usize::from(register)])
}

fn scalar_output(
    program: &VisualProgram,
    registers: &[[f32; 4]; MAX_VISUAL_INSTRUCTIONS],
    output_kind: VisualOutput,
    fallback: f32,
) -> f32 {
    let value = output(program, registers, output_kind).map_or(fallback, |output| output[0]);
    finite_component(value, fallback)
}

fn finite_component(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn finite_or(value: [f32; 4], fallback: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|index| finite_component(value[index], fallback[index]))
}

fn bounded_offset(value: [f32; 4], maximum: f32) -> [f32; 3] {
    let mut vector = [
        finite_component(value[0], 0.0),
        finite_component(value[1], 0.0),
        finite_component(value[2], 0.0),
    ];
    let length_squared = vector
        .iter()
        .map(|component| component * component)
        .sum::<f32>();
    if length_squared > maximum * maximum && length_squared > 0.0 {
        let scale = maximum / length_squared.sqrt();
        for component in &mut vector {
            *component *= scale;
        }
    }
    vector
}
