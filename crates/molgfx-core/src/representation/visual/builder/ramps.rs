//! Sampling an existing colour ramp from inside a program.

use super::super::{ColorExpr, ScalarExpr, VisualError};
use super::VisualProgramBuilder;
use crate::ScalarRamp;

impl VisualProgramBuilder {
    /// Samples an existing reversible scalar ramp of any stop count.
    ///
    /// Costs three program instructions per stop, so the program's fixed
    /// instruction budget bounds the stops a program can carry.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the value came from a different builder,
    /// when a ramp stop is not finite, or when the program has no instruction
    /// slot left.
    pub fn ramp(&mut self, value: ScalarExpr, ramp: ScalarRamp) -> Result<ColorExpr, VisualError> {
        let values = ramp.values();
        let colors = ramp.colors();
        let mut previous_stop = self.scalar(values[0])?;
        let mut blended = self.color(colors[0].to_f32())?;
        for (stop, color) in values.iter().zip(colors).skip(1) {
            let next_stop = self.scalar(*stop)?;
            let weight = self.smoothstep(previous_stop, next_stop, value)?;
            let next_color = self.color(color.to_f32())?;
            blended = self.mix_color(blended, next_color, weight)?;
            previous_stop = next_stop;
        }
        Ok(blended)
    }
}
