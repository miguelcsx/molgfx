//! Literal values a program can name directly.

use super::super::compiler::finite;
use super::super::{
    BoolExpr, ColorExpr, Opcode, ScalarExpr, ValueKind, VectorExpr, VisualError, VisualStage,
};
use super::VisualProgramBuilder;

impl VisualProgramBuilder {
    /// Literal scalar.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the value is not finite, or when the
    /// program has no instruction slot left.
    pub fn scalar(&mut self, value: f32) -> Result<ScalarExpr, VisualError> {
        finite(&[value])?;
        self.emit_at(
            ValueKind::Scalar,
            Opcode::Constant,
            [0; 3],
            [value, 0.0, 0.0, 0.0],
            VisualStage::Uniform,
        )
        .map(ScalarExpr)
    }

    /// Literal linear RGBA color.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the value is not finite, or when the
    /// program has no instruction slot left.
    pub fn color(&mut self, value: [f32; 4]) -> Result<ColorExpr, VisualError> {
        finite(&value)?;
        self.emit_at(
            ValueKind::Color,
            Opcode::Constant,
            [0; 3],
            value,
            VisualStage::Uniform,
        )
        .map(ColorExpr)
    }

    /// Literal vector.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the value is not finite, or when the
    /// program has no instruction slot left.
    pub fn vector(&mut self, value: [f32; 3]) -> Result<VectorExpr, VisualError> {
        finite(&value)?;
        self.emit_at(
            ValueKind::Vector,
            Opcode::Constant,
            [0; 3],
            [value[0], value[1], value[2], 0.0],
            VisualStage::Uniform,
        )
        .map(VectorExpr)
    }

    /// Literal boolean.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the value is not finite, or when the
    /// program has no instruction slot left.
    pub fn boolean(&mut self, value: bool) -> Result<BoolExpr, VisualError> {
        self.emit_at(
            ValueKind::Bool,
            Opcode::Constant,
            [0; 3],
            [f32::from(u8::from(value)), 0.0, 0.0, 0.0],
            VisualStage::Uniform,
        )
        .map(BoolExpr)
    }
}
