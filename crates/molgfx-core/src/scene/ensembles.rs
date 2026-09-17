//! Stable weighted-ensemble storage and lifecycle validation.

use crate::{CoreError, Ensemble, EnsembleHandle, Scene};

#[cfg(test)]
#[path = "ensembles_tests.rs"]
mod tests;

impl Scene {
    /// Adds an ensemble after resolving every placed-structure member.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] if any member is absent.
    pub fn add_ensemble(&mut self, ensemble: Ensemble) -> Result<EnsembleHandle, CoreError> {
        if ensemble
            .members()
            .iter()
            .any(|member| self.structure(*member).is_none())
        {
            return Err(CoreError::StaleHandle);
        }
        self.ensemble_revision = self.ensemble_revision.wrapping_add(1);
        Ok(EnsembleHandle(self.ensembles.insert(ensemble)))
    }

    /// Resolves a weighted ensemble.
    #[must_use]
    pub fn ensemble(&self, handle: EnsembleHandle) -> Option<&Ensemble> {
        self.ensembles.get(handle.0)
    }

    /// Removes an ensemble and invalidates its handle.
    pub fn remove_ensemble(&mut self, handle: EnsembleHandle) -> Option<Ensemble> {
        let removed = self.ensembles.remove(handle.0);
        if removed.is_some() {
            self.ensemble_revision = self.ensemble_revision.wrapping_add(1);
        }
        removed
    }

    /// Iterates ensembles in stable slot order.
    pub fn ensembles(&self) -> impl Iterator<Item = (EnsembleHandle, &Ensemble)> + '_ {
        self.ensembles
            .iter()
            .map(|(handle, ensemble)| (EnsembleHandle(handle), ensemble))
    }

    /// Revision of weighted membership state.
    #[must_use]
    pub const fn ensemble_revision(&self) -> u64 {
        self.ensemble_revision
    }
}
