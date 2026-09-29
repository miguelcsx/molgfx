//! Evaluated selection rows, remembered by query and molecule identity.
//!
//! Evaluating a query is the expensive part of an edit — on a large structure
//! it dominates everything else a patch does — and a scene evaluates the same
//! few queries over and over: every structural resolution re-derives every
//! representation's selection, and appearance rules and retargeted layers ask
//! for rows too. Rows depend only on the structure's identity, its coordinate
//! revision (spatial predicates read coordinates) and the normalized query, so
//! that triple is the key. An unchanged query over an unchanged molecule is
//! evaluated once.
//!
//! The cache is bounded. It holds at most [`SelectionRows::CAPACITY`] entries
//! and evicts the least recently used, deterministically. Lookups take a lock
//! so that preparing a patch can stay a read-only operation on the scene.

use crate::error::Error;
use crate::id::StructureId;
use crate::selection::Selection;
use molgfx_core::{AtomSelection, MolecularSource};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Identity of one evaluation: structure, source identity, coordinate
/// revision and query fingerprint.
type RowKey = (StructureId, u64, u64, u64);

/// Remembered rows for recently evaluated queries.
#[derive(Debug, Default)]
pub(crate) struct SelectionRows {
    /// Most recently used last.
    entries: Mutex<VecDeque<(RowKey, Arc<AtomSelection>)>>,
}

impl SelectionRows {
    /// Distinct evaluations remembered at once.
    pub(crate) const CAPACITY: usize = 256;

    /// The rows `selection` selects in `source`, and the query's fingerprint.
    ///
    /// # Errors
    ///
    /// Returns an error when the query does not compile or evaluate.
    pub(crate) fn rows(
        &self,
        structure: StructureId,
        source: &MolecularSource,
        selection: &Selection,
    ) -> Result<(Arc<AtomSelection>, u64), Error> {
        let query = selection.compiled()?;
        let fingerprint = query.fingerprint().get();
        let key = (
            structure,
            source.identity(),
            source.coordinate_revision(),
            fingerprint,
        );
        if let Ok(mut entries) = self.entries.lock()
            && let Some(index) = entries.iter().position(|(known, _)| *known == key)
            && let Some(entry) = entries.remove(index)
        {
            let rows = Arc::clone(&entry.1);
            entries.push_back(entry);
            return Ok((rows, fingerprint));
        }
        let rows = Arc::new(source.select_compiled(&query)?);
        // A poisoned lock only loses reuse; the evaluated rows are still right.
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= Self::CAPACITY {
                let _ = entries.pop_front();
            }
            entries.push_back((key, Arc::clone(&rows)));
        }
        Ok((rows, fingerprint))
    }

    /// Number of remembered evaluations.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        match self.entries.lock() {
            Ok(entries) => entries.len(),
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
#[path = "selection_rows_tests.rs"]
mod tests;
