//! Weighted caller-declared structural ensembles.

use crate::{CoreError, StructureHandle};
use std::sync::Arc;

#[cfg(test)]
#[path = "ensemble_tests.rs"]
mod tests;

/// Stable members and normalized populations for one ensemble.
#[derive(Clone, PartialEq, Debug)]
pub struct Ensemble {
    members: Arc<[StructureHandle]>,
    weights: Arc<[f32]>,
    provenance: Arc<str>,
    dominant: usize,
}

impl Ensemble {
    /// Validates and normalizes positive caller populations.
    ///
    /// # Errors
    ///
    /// Members and weights must be non-empty, equally sized, unique, finite
    /// and strictly positive; provenance must be non-empty.
    pub fn new(
        members: Arc<[StructureHandle]>,
        weights: &[f32],
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        let provenance = provenance.into();
        if members.is_empty() || members.len() != weights.len() {
            return Err(invalid(
                "members and weights must have the same non-zero length",
            ));
        }
        if provenance.trim().is_empty() {
            return Err(invalid("ensemble provenance must be non-empty"));
        }
        if weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
        {
            return Err(invalid(
                "ensemble weights must be finite and strictly positive",
            ));
        }
        if members
            .iter()
            .enumerate()
            .any(|(index, member)| members[..index].contains(member))
        {
            return Err(invalid("ensemble members must be unique placements"));
        }
        let sum = weights.iter().sum::<f32>();
        if !sum.is_finite() || sum <= 0.0 {
            return Err(invalid("ensemble weight sum must be finite and positive"));
        }
        let weights = weights
            .iter()
            .map(|weight| weight / sum)
            .collect::<Arc<[_]>>();
        let dominant = weights
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1).then_with(|| right.0.cmp(&left.0)))
            .map_or(0, |(index, _)| index);
        Ok(Self {
            members,
            weights,
            provenance,
            dominant,
        })
    }

    /// Members in deterministic caller order.
    #[must_use]
    pub fn members(&self) -> &[StructureHandle] {
        &self.members
    }

    /// Normalized probabilities/populations summing to one within float error.
    #[must_use]
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Caller computation or dataset identifier.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Stable index of the highest-weight member; earliest wins ties.
    #[must_use]
    pub const fn dominant_index(&self) -> usize {
        self.dominant
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidEnsemble { reason }
}
