//! Parameters a caller rebinds without recompiling the program.
//!
//! A parameter is the difference between a program that must be rebuilt to
//! change a threshold and one whose threshold is a uniform write.

use super::super::compiler::finite;
use super::super::{
    ColorExpr, ColorParameter, ScalarExpr, ScalarParameter, ValueKind, VectorExpr, VectorParameter,
    VisualError,
};
use super::VisualProgramBuilder;

impl VisualProgramBuilder {
    /// Declares a scalar parameter and its default.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the default is not finite, or when the
    /// program already declares its limit of parameters.
    pub fn scalar_parameter(
        &mut self,
        default: f32,
    ) -> Result<(ScalarParameter, ScalarExpr), VisualError> {
        finite(&[default])?;
        let parameter = self.parameter(ValueKind::Scalar, [default, 0.0, 0.0, 0.0])?;
        let expression = self.parameter_input(parameter, ValueKind::Scalar)?;
        Ok((ScalarParameter(parameter), ScalarExpr(expression)))
    }

    /// Declares a color parameter and its default.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the default is not finite, or when the
    /// program already declares its limit of parameters.
    pub fn color_parameter(
        &mut self,
        default: [f32; 4],
    ) -> Result<(ColorParameter, ColorExpr), VisualError> {
        finite(&default)?;
        let parameter = self.parameter(ValueKind::Color, default)?;
        let expression = self.parameter_input(parameter, ValueKind::Color)?;
        Ok((ColorParameter(parameter), ColorExpr(expression)))
    }

    /// Declares a vector parameter and its default.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the default is not finite, or when the
    /// program already declares its limit of parameters.
    pub fn vector_parameter(
        &mut self,
        default: [f32; 3],
    ) -> Result<(VectorParameter, VectorExpr), VisualError> {
        finite(&default)?;
        let packed = [default[0], default[1], default[2], 0.0];
        let parameter = self.parameter(ValueKind::Vector, packed)?;
        let expression = self.parameter_input(parameter, ValueKind::Vector)?;
        Ok((VectorParameter(parameter), VectorExpr(expression)))
    }
}
