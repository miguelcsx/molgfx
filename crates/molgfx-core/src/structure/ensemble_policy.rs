//! How an ensemble's weights become member opacities.
//!
//! One definition serves the declarative scene preset, the generic composition
//! and the validated [`super::Ensemble`], so a weight means the same thing
//! wherever it is drawn.

use crate::CoreError;

/// Opacity bounds for the members of a weighted overlay.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EnsembleOpacity {
    /// Opacity of the highest-weight member.
    pub dominant_opacity: f32,
    /// Opacity of the other members when their weight equals the dominant one.
    pub alternate_opacity: f32,
    /// Floor that keeps a light member visible.
    pub minimum_opacity: f32,
}

impl Default for EnsembleOpacity {
    fn default() -> Self {
        Self {
            dominant_opacity: 1.0,
            alternate_opacity: 0.55,
            minimum_opacity: 0.08,
        }
    }
}

impl EnsembleOpacity {
    /// Checks `minimum <= alternate <= dominant`, all within zero to one.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidEnsemble`] for a value that is not finite or
    /// out of order.
    pub fn validate(self) -> Result<(), CoreError> {
        let finite = [
            self.dominant_opacity,
            self.alternate_opacity,
            self.minimum_opacity,
        ]
        .iter()
        .all(|value| value.is_finite());
        let ordered = (0.0..=1.0).contains(&self.minimum_opacity)
            && (self.minimum_opacity..=self.dominant_opacity).contains(&self.alternate_opacity)
            && (self.alternate_opacity..=1.0).contains(&self.dominant_opacity);
        if finite && ordered {
            Ok(())
        } else {
            Err(CoreError::InvalidEnsemble {
                reason: "opacities must satisfy 0 <= minimum <= alternate <= dominant <= 1",
            })
        }
    }
}

/// The index of the largest weight; the earliest wins a tie.
#[must_use]
pub fn dominant_index(weights: &[f32]) -> usize {
    weights.iter().enumerate().fold(0, |best, (index, weight)| {
        if weight.total_cmp(&weights[best]).is_gt() {
            index
        } else {
            best
        }
    })
}

/// The opacity of each member for the given relative `weights`.
///
/// The heaviest member draws at `dominant_opacity`; every other member's
/// opacity is its weight relative to that one times `alternate_opacity`,
/// clamped to `[minimum_opacity, alternate_opacity]`. Only the ratios between
/// weights matter.
///
/// # Errors
///
/// Returns [`CoreError::InvalidEnsemble`] for no weights, a weight that is
/// negative or not finite, weights that are all zero, or invalid opacities.
pub fn ensemble_opacities(
    weights: &[f32],
    opacity: EnsembleOpacity,
) -> Result<Vec<f32>, CoreError> {
    opacity.validate()?;
    if weights.is_empty() {
        return Err(invalid("an ensemble needs at least one member"));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight < 0.0)
    {
        return Err(invalid("ensemble weights must be finite and non-negative"));
    }
    if weights.iter().all(|weight| *weight == 0.0) {
        return Err(invalid("ensemble weights must not all be zero"));
    }
    let dominant = dominant_index(weights);
    let heaviest = weights[dominant];
    Ok(weights
        .iter()
        .enumerate()
        .map(|(index, weight)| {
            if index == dominant {
                opacity.dominant_opacity
            } else {
                (weight / heaviest * opacity.alternate_opacity)
                    .clamp(opacity.minimum_opacity, opacity.alternate_opacity)
            }
        })
        .collect())
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidEnsemble { reason }
}

#[cfg(test)]
#[path = "ensemble_policy_tests.rs"]
mod tests;
