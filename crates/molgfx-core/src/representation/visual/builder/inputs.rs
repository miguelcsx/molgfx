//! Values the drawable resolves before a program runs.
//!
//! Every one of these is read from the shading context rather than
//! computed, so a program that only reads them costs one instruction each
//! and nothing at evaluation time beyond a load.

use super::super::{BoolExpr, ColorExpr, Opcode, ScalarExpr, ValueKind, VectorExpr, VisualError};
use super::VisualProgramBuilder;

impl VisualProgramBuilder {
    /// Tests one GPU-resident semantic interaction bit.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the mask is empty or the program has no
    /// instruction slot left.
    pub fn interaction_state(&mut self, mask: u32) -> Result<BoolExpr, VisualError> {
        if mask == 0 {
            return Err(VisualError::MalformedProgram);
        }
        self.emit_at(
            ValueKind::Bool,
            Opcode::State,
            [0; 3],
            [f32::from_bits(mask), 0.0, 0.0, 0.0],
            super::super::VisualStage::Entity,
        )
        .map(BoolExpr)
    }

    /// Built-in color resolved by the drawable before this program runs.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn base_color(&mut self) -> Result<ColorExpr, VisualError> {
        self.input(ValueKind::Color, 0).map(ColorExpr)
    }

    /// Built-in opacity resolved by the drawable before this program runs.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn base_opacity(&mut self) -> Result<ScalarExpr, VisualError> {
        self.input(ValueKind::Scalar, 1).map(ScalarExpr)
    }

    /// Current scene/timeline time in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn time(&mut self) -> Result<ScalarExpr, VisualError> {
        self.input(ValueKind::Scalar, 2).map(ScalarExpr)
    }

    /// Drawable-local position.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn local_position(&mut self) -> Result<VectorExpr, VisualError> {
        self.input(ValueKind::Vector, 3).map(VectorExpr)
    }

    /// World-space shaded position.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn world_position(&mut self) -> Result<VectorExpr, VisualError> {
        self.input(ValueKind::Vector, 4).map(VectorExpr)
    }

    /// World-space geometric normal.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn normal(&mut self) -> Result<VectorExpr, VisualError> {
        self.input(ValueKind::Vector, 5).map(VectorExpr)
    }

    /// Normalized direction from the shaded point toward the camera.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn view_direction(&mut self) -> Result<VectorExpr, VisualError> {
        self.input(ValueKind::Vector, 6).map(VectorExpr)
    }

    /// World-space distance from the shaded point to the camera.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn camera_distance(&mut self) -> Result<ScalarExpr, VisualError> {
        self.input(ValueKind::Scalar, 7).map(ScalarExpr)
    }

    /// Stable entity row converted exactly to a scalar while representable.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn entity_index(&mut self) -> Result<ScalarExpr, VisualError> {
        self.input(ValueKind::Scalar, 8).map(ScalarExpr)
    }

    /// Built-in material roughness.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn base_roughness(&mut self) -> Result<ScalarExpr, VisualError> {
        self.input(ValueKind::Scalar, 9).map(ScalarExpr)
    }

    /// Built-in material specular strength.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn base_specular(&mut self) -> Result<ScalarExpr, VisualError> {
        self.input(ValueKind::Scalar, 10).map(ScalarExpr)
    }

    /// Built-in model-specific material strength.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no instruction slot left.
    pub fn base_material_strength(&mut self) -> Result<ScalarExpr, VisualError> {
        self.input(ValueKind::Scalar, 11).map(ScalarExpr)
    }
}
