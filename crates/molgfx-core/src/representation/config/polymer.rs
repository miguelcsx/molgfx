//! Independent polymer widths, fitted helix radii and cross-section controls.
use super::RepresentationConfig;
use crate::CoreError;

impl RepresentationConfig {
    /// Sets cartoon ribbon width in ångström.
    #[must_use]
    pub const fn ribbon_width(mut self, width: f32) -> Self {
        self.params.ribbon_width = width;
        self
    }

    /// Sets the radius of fitted circular helix columns independently of beta width.
    ///
    /// # Errors
    /// Rejects nonpositive, nonfinite or unrepresentable diameters.
    pub fn cartoon_helix_radius(mut self, radius: f32) -> Result<Self, CoreError> {
        if !radius.is_finite() || radius <= 0.0 || !(radius * 2.0).is_finite() {
            return Err(CoreError::InvalidProperty {
                reason: "cartoon helix radius must be finite and positive",
            });
        }
        self.params.cartoon_helix_radius = radius;
        Ok(self)
    }

    /// Sets secondary-structure cross-section proportions and arrow shoulders.
    ///
    /// # Errors
    /// The aspect ratio must be finite and positive; the arrow factor must be
    /// finite and non-negative. Zero retains the unmodified strand body.
    pub fn cartoon_shape(
        mut self,
        aspect_ratio: f32,
        arrow_factor: f32,
    ) -> Result<Self, CoreError> {
        if !aspect_ratio.is_finite()
            || aspect_ratio <= 0.0
            || !arrow_factor.is_finite()
            || arrow_factor < 0.0
        {
            return Err(CoreError::InvalidProperty {
                reason: "cartoon aspect ratio must be positive and arrow factor non-negative, both finite",
            });
        }
        self.params.cartoon_aspect_ratio = aspect_ratio;
        self.params.cartoon_arrow_factor = arrow_factor;
        Ok(self)
    }

    /// Selects connectors across real polymer gaps.
    #[must_use]
    pub const fn gaps(mut self, style: crate::GapStyle) -> Self {
        self.params.gaps = style;
        self
    }

    /// Enables source-anchored polymer direction wedges.
    #[must_use]
    pub const fn direction_wedges(mut self, enabled: bool) -> Self {
        self.params.cartoon_direction_wedges = enabled;
        self
    }

    /// Selects the protein helix and nucleic backbone cross-sections.
    #[must_use]
    pub const fn cartoon_profiles(
        mut self,
        helix: crate::CartoonProfile,
        nucleic: crate::CartoonProfile,
    ) -> Self {
        self.params.cartoon_helix_profile = helix;
        self.params.cartoon_nucleic_profile = nucleic;
        self
    }
}
