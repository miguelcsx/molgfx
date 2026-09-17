//! Typed output assignments kept separate from graph construction.

use super::{
    BoolExpr, ColorExpr, ScalarExpr, VectorExpr, VisualError, VisualOutput, VisualProgram,
    VisualProgramBuilder,
};

impl VisualProgramBuilder {
    /// Sets a color expression and freezes the program in one operation.
    ///
    /// This is the concise path for the common case where a visual recipe only
    /// computes color. Multi-output recipes use the individual setters before
    /// calling [`Self::finish`].
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from another builder
    /// or final graph validation fails.
    pub fn finish_color(mut self, value: ColorExpr) -> Result<VisualProgram, VisualError> {
        self.set_base_color(value)?;
        self.finish()
    }

    /// Sets the output color.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_base_color(&mut self, value: ColorExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::BaseColor, value.0)
    }
    /// Sets the output opacity.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_opacity(&mut self, value: ScalarExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::Opacity, value.0)
    }
    /// Sets additive HDR emission.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_emission(&mut self, value: ColorExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::Emission, value.0)
    }
    /// Sets perceptual roughness.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_roughness(&mut self, value: ScalarExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::Roughness, value.0)
    }
    /// Sets specular strength.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_specular(&mut self, value: ScalarExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::Specular, value.0)
    }
    /// Sets the selected material model's scalar strength.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_material_strength(&mut self, value: ScalarExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::MaterialStrength, value.0)
    }
    /// Sets visibility; false discards consistently in every pass.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_visibility(&mut self, value: BoolExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::Visibility, value.0)
    }
    /// Sets analytic silhouette softness in physical pixels.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_silhouette_softness(&mut self, value: ScalarExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::SilhouetteSoftness, value.0)
    }
    /// Sets analytic radius scaling.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_radius_scale(&mut self, value: ScalarExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::RadiusScale, value.0)
    }
    /// Sets line, ribbon or capsule width scaling.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the expression came from a different
    /// builder, when the channel was already assigned, or when the channel is
    /// not one the target drawable family supports.
    pub fn set_width_scale(&mut self, value: ScalarExpr) -> Result<(), VisualError> {
        self.output(VisualOutput::WidthScale, value.0)
    }
    /// Sets bounded local displacement and its conservative culling bound.
    ///
    /// The bound is what culling widens every drawable's extent by, so a
    /// program that moves a fragment further than it declared would be culled
    /// while still visible.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the bound is negative or not finite, when
    /// the expression came from a different builder, or when the channel was
    /// already assigned.
    pub fn set_position_offset(
        &mut self,
        value: VectorExpr,
        maximum_displacement: f32,
    ) -> Result<(), VisualError> {
        if !maximum_displacement.is_finite() || maximum_displacement < 0.0 {
            return Err(VisualError::InvalidRange);
        }
        self.output(VisualOutput::PositionOffset, value.0)?;
        self.maximum_displacement = maximum_displacement;
        Ok(())
    }
}
