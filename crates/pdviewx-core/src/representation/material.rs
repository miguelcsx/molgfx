//! Typed, finite surface-response state shared by all representations.

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;

/// Surface response of a drawn representation.
///
/// The restrained molecular response is the scientific default. Other tagged
/// models are explicit art direction and never inferred from an element name.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Material {
    /// Overall opacity in [0, 1]; below 1 the representation draws in the
    /// translucent pass.
    pub opacity: f32,
    /// Perceptual micro-surface roughness in [0, 1].
    pub roughness: f32,
    /// Specular strength in [0, 1]; kept low, molecules are not chrome.
    pub specular: f32,
    /// Lighting response model. This is presentation state, not chemistry.
    pub model: MaterialModel,
}

/// Tagged lighting response selected per representation.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum MaterialModel {
    /// Restrained dielectric response for scientific inspection.
    #[default]
    Molecular,
    /// Energy-conserving metalness workflow for explicit art direction.
    Principled {
        /// Metalness in `[0, 1]`; pure surfaces normally use an endpoint.
        metallic: f32,
    },
    /// Tangent-aligned dielectric response for polymer ribbons.
    AnisotropicRibbon {
        /// Lobe elongation in `[0, 1]`; zero resolves to isotropic dielectric.
        strength: f32,
    },
    /// Bounded screen-space diffusion cue for explicit art direction.
    Diffusion {
        /// Scattering strength in `[0, 1]`; this is not a measured radius.
        strength: f32,
    },
}

impl Default for Material {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            // A polished dielectric rather than a matte one. A tight highlight
            // is what lets a curved impostor read as a solid body instead of a
            // flat disc, so the default sits where a wet biological surface
            // does, not where dry plastic does.
            roughness: 0.34,
            specular: 0.5,
            model: MaterialModel::Molecular,
        }
    }
}

impl Material {
    /// Finite roughness consumed by lighting; malformed input resolves neutral.
    #[must_use]
    pub fn perceptual_roughness(self) -> f32 {
        if self.roughness.is_finite() {
            self.roughness.clamp(0.05, 0.92)
        } else {
            Self::default().roughness
        }
    }

    /// Finite dielectric highlight strength consumed by the lighting model.
    #[must_use]
    pub fn specular_strength(self) -> f32 {
        if self.specular.is_finite() {
            self.specular.clamp(0.0, 1.0)
        } else {
            Self::default().specular
        }
    }

    /// Finite principled metalness, or zero for another model.
    #[must_use]
    pub fn metallic(self) -> f32 {
        match self.model {
            MaterialModel::Principled { metallic } if metallic.is_finite() => {
                metallic.clamp(0.0, 1.0)
            }
            MaterialModel::Molecular
            | MaterialModel::Principled { .. }
            | MaterialModel::AnisotropicRibbon { .. }
            | MaterialModel::Diffusion { .. } => 0.0,
        }
    }

    /// Finite tangent-aligned response strength, or zero for another model.
    #[must_use]
    pub fn anisotropy(self) -> f32 {
        match self.model {
            MaterialModel::AnisotropicRibbon { strength } if strength.is_finite() => {
                strength.clamp(0.0, 1.0)
            }
            MaterialModel::Molecular
            | MaterialModel::Principled { .. }
            | MaterialModel::AnisotropicRibbon { .. }
            | MaterialModel::Diffusion { .. } => 0.0,
        }
    }

    /// Finite bounded diffusion response, or zero for another model.
    #[must_use]
    pub fn diffusion_strength(self) -> f32 {
        match self.model {
            MaterialModel::Diffusion { strength } if strength.is_finite() => {
                strength.clamp(0.0, 1.0)
            }
            MaterialModel::Molecular
            | MaterialModel::Principled { .. }
            | MaterialModel::AnisotropicRibbon { .. }
            | MaterialModel::Diffusion { .. } => 0.0,
        }
    }

    /// Compact shader tag and model parameter.
    #[must_use]
    pub fn model_lanes(self) -> [f32; 2] {
        match self.model {
            MaterialModel::Molecular => [0.0, 0.0],
            MaterialModel::Principled { .. } => [1.0, self.metallic()],
            MaterialModel::AnisotropicRibbon { .. } => [2.0, self.anisotropy()],
            MaterialModel::Diffusion { .. } => [3.0, self.diffusion_strength()],
        }
    }

    /// Creates an explicit art-directed principled material.
    #[must_use]
    pub fn principled(metallic: f32) -> Self {
        Self {
            model: MaterialModel::Principled { metallic },
            ..Self::default()
        }
    }

    /// Creates an explicit tangent-aligned ribbon material.
    #[must_use]
    pub fn anisotropic_ribbon(strength: f32) -> Self {
        Self {
            model: MaterialModel::AnisotropicRibbon { strength },
            ..Self::default()
        }
    }

    /// Creates an explicit, bounded diffusion presentation.
    #[must_use]
    pub fn diffusion(strength: f32) -> Self {
        Self {
            model: MaterialModel::Diffusion { strength },
            ..Self::default()
        }
    }

    /// True when this material belongs in the order-independent translucent pass.
    #[must_use]
    pub fn is_translucent(self) -> bool {
        self.opacity.is_finite() && self.opacity.clamp(0.0, 1.0) < 1.0
    }

    /// Opacity quantized to unorm8 with deterministic round-to-nearest conversion.
    #[must_use]
    pub fn opacity_unorm8(self) -> u8 {
        if !self.opacity.is_finite() {
            return u8::MAX;
        }
        let target = self.opacity.clamp(0.0, 1.0) * 255.0;
        let mut low = 0u16;
        let mut high = u16::from(u8::MAX);
        while low < high {
            let middle = (low + high).div_ceil(2);
            if f32::from(middle) <= target {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let lower = low;
        let upper = (low + 1).min(u16::from(u8::MAX));
        let selected = if target - f32::from(lower) < f32::from(upper) - target {
            lower
        } else {
            upper
        };
        u8::try_from(selected).map_or(u8::MAX, |value| value)
    }
}
