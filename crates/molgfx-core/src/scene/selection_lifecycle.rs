//! Adding query-evaluated selections and releasing ones nothing draws.
//!
//! A semantic layer that evaluates a query itself hands the rows here together
//! with the query's fingerprint, so the renderer recognizes two selections of
//! the same query and shares what it derives from them. When a representation
//! is retargeted in place, the selection it used to draw is released rather
//! than left to accumulate.

use crate::handle::{SelectionHandle, StructureHandle};
use crate::{AtomSelection, CoreError, RepresentationTarget, Scene};

impl Scene {
    /// Stores rows of one structure evaluated from a normalized query.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure.
    pub fn add_query_selection(
        &mut self,
        structure: StructureHandle,
        selection: AtomSelection,
        query_fingerprint: u64,
    ) -> Result<SelectionHandle, CoreError> {
        if self.structures.get(structure.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        Ok(self.add_scoped_selection_with_fingerprint(
            vec![(structure, selection)],
            Some(query_fingerprint),
        ))
    }

    /// Releases a selection that no representation targets.
    ///
    /// Runtime is linear in the number of representations, which are checked
    /// so that a selection still being drawn is never released.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown selection, and
    /// [`CoreError::InvalidSelection`] while a representation still draws it.
    pub fn remove_selection(&mut self, handle: SelectionHandle) -> Result<(), CoreError> {
        if self.selections.get(handle.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let in_use = self
            .representations
            .iter()
            .any(|(_, stored)| stored.value.target == RepresentationTarget::Selection(handle));
        if in_use {
            return Err(CoreError::InvalidSelection {
                reason: "a representation still draws this selection",
            });
        }
        let _ = self.selections.remove(handle.0);
        Ok(())
    }
}

#[cfg(test)]
#[path = "selection_lifecycle_tests.rs"]
mod tests;
