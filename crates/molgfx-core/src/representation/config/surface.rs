//! Explicit density sampling controls shared by molecular surface recipes.
use super::RepresentationConfig;
impl RepresentationConfig {
    /// Scales atomic radii into Gaussian standard deviations.
    #[must_use]
    pub const fn gaussian_sigma(mut self, sigma: f32) -> Self {
        self.params.gaussian_sigma = sigma;
        self
    }
    /// Limits Gaussian support to this many standard deviations.
    #[must_use]
    pub const fn gaussian_cutoff(mut self, cutoff: f32) -> Self {
        self.params.gaussian_cutoff = cutoff;
        self
    }
    /// Requests grid spacing in ångström, subject to device and memory limits.
    #[must_use]
    pub const fn surface_spacing(mut self, spacing: f32) -> Self {
        self.params.surface_spacing = Some(spacing);
        self
    }
}
