//! Reversible property mapping for variable-radius spline tubes.

use crate::CoreError;

#[cfg(test)]
#[path = "tube_radius_tests.rs"]
mod tests;

/// Radius policy along trace/tube splines.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum TubeRadiusMapping {
    /// Use [`crate::RepresentationParams::tube_radius`] everywhere.
    #[default]
    Constant,
    /// Map recorded crystallographic B factors linearly to radii in Ångström.
    BFactor {
        /// Inclusive B-factor domain.
        domain: [f32; 2],
        /// Radii corresponding to the domain endpoints, Ångström.
        radii: [f32; 2],
    },
}

impl TubeRadiusMapping {
    /// Returns the serialized B-factor domain and endpoint radii.
    #[must_use]
    pub const fn b_factor_parameters(self) -> Option<([f32; 2], [f32; 2])> {
        match self {
            Self::Constant => None,
            Self::BFactor { domain, radii } => Some((domain, radii)),
        }
    }

    /// Creates a monotonic, reversible B-factor-to-radius mapping.
    ///
    /// # Errors
    ///
    /// Domain endpoints must increase and radii must be finite, positive and
    /// unequal. Increasing and decreasing radius mappings are both valid.
    pub fn b_factor(domain: [f32; 2], radii: [f32; 2]) -> Result<Self, CoreError> {
        if !domain
            .iter()
            .chain(radii.iter())
            .all(|value| value.is_finite())
            || domain[0] >= domain[1]
            || radii.iter().any(|&radius| radius <= 0.0)
            || (radii[0] - radii[1]).abs() <= f32::EPSILON
        {
            return Err(CoreError::InvalidProperty {
                reason: "tube radius mapping requires an increasing domain and distinct positive radii",
            });
        }
        Ok(Self::BFactor { domain, radii })
    }

    /// Maps one recorded value, clamping outside the declared domain.
    #[must_use]
    pub fn radius(self, value: f32, fallback: f32) -> f32 {
        match self {
            Self::Constant => fallback,
            Self::BFactor { domain, radii } => {
                let amount = ((value - domain[0]) / (domain[1] - domain[0])).clamp(0.0, 1.0);
                radii[0] + amount * (radii[1] - radii[0])
            }
        }
    }

    /// Inverts a mapped radius back to its source value, clamped to domain.
    #[must_use]
    pub fn value(self, radius: f32) -> Option<f32> {
        let Self::BFactor { domain, radii } = self else {
            return None;
        };
        let amount = ((radius - radii[0]) / (radii[1] - radii[0])).clamp(0.0, 1.0);
        Some(domain[0] + amount * (domain[1] - domain[0]))
    }
}
