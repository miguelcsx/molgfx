//! Sampling an existing colour ramp from inside a program.

use super::super::{ColorExpr, ScalarExpr, VisualError};
use super::VisualProgramBuilder;
use crate::ScalarRamp;

impl VisualProgramBuilder {
    /// Samples an existing reversible three-stop scalar ramp.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the value came from a different builder,
    /// when a ramp stop is not finite, or when the program has no instruction
    /// slot left.
    pub fn ramp(&mut self, value: ScalarExpr, ramp: ScalarRamp) -> Result<ColorExpr, VisualError> {
        let values = ramp.values();
        let colors = ramp.colors();
        let low = self.scalar(values[0])?;
        let middle = self.scalar(values[1])?;
        let high = self.scalar(values[2])?;
        let first_weight = self.smoothstep(low, middle, value)?;
        let second_weight = self.smoothstep(middle, high, value)?;
        let first = self.color(colors[0].to_f32())?;
        let second = self.color(colors[1].to_f32())?;
        let third = self.color(colors[2].to_f32())?;
        let lower = self.mix_color(first, second, first_weight)?;
        self.mix_color(lower, third, second_weight)
    }
}
