//! Comparison, boolean algebra and branchless selection.
//!
//! Selection evaluates both sides and picks one. A program runs in lockstep
//! across a warp, so a real branch would cost both paths anyway and a select
//! keeps the instruction stream uniform.

use super::super::{BoolExpr, ColorExpr, Opcode, ScalarExpr, ValueKind, VisualError};
use super::VisualProgramBuilder;

impl VisualProgramBuilder {
    /// Scalar less-than comparison.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn less(&mut self, left: ScalarExpr, right: ScalarExpr) -> Result<BoolExpr, VisualError> {
        self.binary(ValueKind::Bool, Opcode::Less, left.0, right.0)
            .map(BoolExpr)
    }

    /// Scalar greater-than comparison.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn greater(
        &mut self,
        left: ScalarExpr,
        right: ScalarExpr,
    ) -> Result<BoolExpr, VisualError> {
        self.binary(ValueKind::Bool, Opcode::Greater, left.0, right.0)
            .map(BoolExpr)
    }

    /// Boolean conjunction.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn and(&mut self, left: BoolExpr, right: BoolExpr) -> Result<BoolExpr, VisualError> {
        self.binary(ValueKind::Bool, Opcode::And, left.0, right.0)
            .map(BoolExpr)
    }

    /// Boolean disjunction.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn or(&mut self, left: BoolExpr, right: BoolExpr) -> Result<BoolExpr, VisualError> {
        self.binary(ValueKind::Bool, Opcode::Or, left.0, right.0)
            .map(BoolExpr)
    }

    /// Boolean negation.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn not(&mut self, value: BoolExpr) -> Result<BoolExpr, VisualError> {
        self.unary(ValueKind::Bool, Opcode::Not, value.0)
            .map(BoolExpr)
    }

    /// Selects one scalar without control flow.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn select_scalar(
        &mut self,
        condition: BoolExpr,
        when_true: ScalarExpr,
        when_false: ScalarExpr,
    ) -> Result<ScalarExpr, VisualError> {
        self.ternary(
            ValueKind::Scalar,
            Opcode::Select,
            condition.0,
            when_true.0,
            when_false.0,
        )
        .map(ScalarExpr)
    }

    /// Selects one color without control flow.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when an operand came from a different builder,
    /// or when the program has no instruction slot left.
    pub fn select_color(
        &mut self,
        condition: BoolExpr,
        when_true: ColorExpr,
        when_false: ColorExpr,
    ) -> Result<ColorExpr, VisualError> {
        self.ternary(
            ValueKind::Color,
            Opcode::Select,
            condition.0,
            when_true.0,
            when_false.0,
        )
        .map(ColorExpr)
    }
}
