//! Scalar, colour and vector arithmetic.
//!
//! Division and normalisation are the guarded forms: a program is evaluated
//! per fragment with no way to report a failure from there, so a degenerate
//! input yields a defined value instead of a NaN that spreads.

use super::super::{ColorExpr, Opcode, ScalarExpr, ValueKind, VectorExpr, VisualError};
use super::VisualProgramBuilder;

impl VisualProgramBuilder {
    /// Adds two scalars.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn add(&mut self, left: ScalarExpr, right: ScalarExpr) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::Add, left.0, right.0)
            .map(ScalarExpr)
    }

    /// Subtracts two scalars.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn subtract(
        &mut self,
        left: ScalarExpr,
        right: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::Subtract, left.0, right.0)
            .map(ScalarExpr)
    }

    /// Multiplies two scalars.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn multiply(
        &mut self,
        left: ScalarExpr,
        right: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::Multiply, left.0, right.0)
            .map(ScalarExpr)
    }

    /// Divides two scalars, returning zero when the denominator is too small.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn safe_divide(
        &mut self,
        left: ScalarExpr,
        right: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::SafeDivide, left.0, right.0)
            .map(ScalarExpr)
    }

    /// Absolute value.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn abs(&mut self, value: ScalarExpr) -> Result<ScalarExpr, VisualError> {
        self.unary(ValueKind::Scalar, Opcode::Abs, value.0)
            .map(ScalarExpr)
    }

    /// Component-independent scalar minimum.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn minimum(
        &mut self,
        left: ScalarExpr,
        right: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::Minimum, left.0, right.0)
            .map(ScalarExpr)
    }

    /// Component-independent scalar maximum.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn maximum(
        &mut self,
        left: ScalarExpr,
        right: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::Maximum, left.0, right.0)
            .map(ScalarExpr)
    }

    /// Clamps a scalar between two expressions.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn clamp(
        &mut self,
        value: ScalarExpr,
        low: ScalarExpr,
        high: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.ternary(ValueKind::Scalar, Opcode::Clamp, value.0, low.0, high.0)
            .map(ScalarExpr)
    }

    /// Clamps a scalar to zero through one.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn saturate(&mut self, value: ScalarExpr) -> Result<ScalarExpr, VisualError> {
        let low = self.scalar(0.0)?;
        let high = self.scalar(1.0)?;
        self.clamp(value, low, high)
    }

    /// Binary threshold.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn step(&mut self, edge: ScalarExpr, value: ScalarExpr) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::Step, edge.0, value.0)
            .map(ScalarExpr)
    }

    /// Hermite interpolation between two edges.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn smoothstep(
        &mut self,
        low: ScalarExpr,
        high: ScalarExpr,
        value: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.ternary(
            ValueKind::Scalar,
            Opcode::SmoothStep,
            low.0,
            high.0,
            value.0,
        )
        .map(ScalarExpr)
    }

    /// Deterministic sine for periodic presentation animation.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn sine(&mut self, value: ScalarExpr) -> Result<ScalarExpr, VisualError> {
        self.unary(ValueKind::Scalar, Opcode::Sine, value.0)
            .map(ScalarExpr)
    }

    /// Linear scalar interpolation.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn mix_scalar(
        &mut self,
        from: ScalarExpr,
        to: ScalarExpr,
        weight: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.ternary(ValueKind::Scalar, Opcode::Mix, from.0, to.0, weight.0)
            .map(ScalarExpr)
    }

    /// Linear color interpolation.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn mix_color(
        &mut self,
        from: ColorExpr,
        to: ColorExpr,
        weight: ScalarExpr,
    ) -> Result<ColorExpr, VisualError> {
        self.ternary(ValueKind::Color, Opcode::Mix, from.0, to.0, weight.0)
            .map(ColorExpr)
    }

    /// Adds two vectors.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn add_vector(
        &mut self,
        left: VectorExpr,
        right: VectorExpr,
    ) -> Result<VectorExpr, VisualError> {
        self.binary(ValueKind::Vector, Opcode::Add, left.0, right.0)
            .map(VectorExpr)
    }

    /// Scales a vector.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn scale_vector(
        &mut self,
        vector: VectorExpr,
        scale: ScalarExpr,
    ) -> Result<VectorExpr, VisualError> {
        self.binary(ValueKind::Vector, Opcode::Multiply, vector.0, scale.0)
            .map(VectorExpr)
    }

    /// Vector dot product.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn dot(&mut self, left: VectorExpr, right: VectorExpr) -> Result<ScalarExpr, VisualError> {
        self.binary(ValueKind::Scalar, Opcode::Dot, left.0, right.0)
            .map(ScalarExpr)
    }

    /// Safe vector normalization; a zero vector remains zero.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn normalize(&mut self, value: VectorExpr) -> Result<VectorExpr, VisualError> {
        self.unary(ValueKind::Vector, Opcode::Normalize, value.0)
            .map(VectorExpr)
    }
}
