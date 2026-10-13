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
//! The cache is bounded by memory, not by entry count. Entries are charged
//! their resident row bytes plus a fixed overhead, and the least recently used
//! are evicted until the total fits [`SelectionRows::BUDGET`]. A scene with
//! thousands of small selections therefore stays fully cached, where a count
//! cap smaller than the working set would evict every entry just before its
//! next use. Recency is an ordered index of use stamps beside a hash map, so a
//! lookup, an insertion and an eviction are each O(log n), never a scan.
//! Lookups take a lock so that preparing a patch can stay a read-only
//! operation on the scene.

use crate::error::Error;
use crate::id::StructureId;
use crate::selection::Selection;
use molgfx_core::{AtomSelection, MolecularSource};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

/// Identity of one evaluation: source identity, coordinate revision and query
/// fingerprint. Rows depend on the molecule alone, so every placed copy of one
/// source shares the evaluation instead of repeating it.
type RowKey = (u64, u64, u64);

/// Bytes charged to every entry on top of its rows: map slot, order slot and
/// the shared pointer.
const ENTRY_OVERHEAD: usize = 96;

#[derive(Debug)]
struct Entry {
    rows: Arc<AtomSelection>,
    stamp: u64,
    cost: usize,
}

#[derive(Debug, Default)]
struct Store {
    entries: HashMap<RowKey, Entry>,
    /// Use stamp to key; the first element is the least recently used.
    order: BTreeMap<u64, RowKey>,
    clock: u64,
    bytes: usize,
}

impl Store {
    fn touch(&mut self, key: RowKey) -> Option<Arc<AtomSelection>> {
        let entry = self.entries.get_mut(&key)?;
        let _ = self.order.remove(&entry.stamp);
        self.clock += 1;
        entry.stamp = self.clock;
        let _ = self.order.insert(self.clock, key);
        Some(Arc::clone(&entry.rows))
    }

    fn insert(&mut self, key: RowKey, rows: Arc<AtomSelection>, budget: usize) {
        let cost = rows.resident_bytes() + ENTRY_OVERHEAD;
        if cost > budget || self.entries.contains_key(&key) {
            return;
        }
        while self.bytes + cost > budget {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&oldest) {
                self.bytes -= evicted.cost;
            }
        }
        self.clock += 1;
        let _ = self.order.insert(self.clock, key);
        let _ = self.entries.insert(
            key,
            Entry {
                rows,
                stamp: self.clock,
                cost,
            },
        );
        self.bytes += cost;
    }
}

/// Remembered rows for recently evaluated queries.
#[derive(Debug)]
pub(crate) struct SelectionRows {
    store: Mutex<Store>,
    budget: usize,
}

impl Default for SelectionRows {
    fn default() -> Self {
        Self::with_budget(Self::BUDGET)
    }
}

impl SelectionRows {
    /// Resident bytes of remembered rows allowed at once.
    pub(crate) const BUDGET: usize = 64 * 1024 * 1024;

    /// A cache that keeps at most `budget` bytes.
    pub(crate) fn with_budget(budget: usize) -> Self {
        Self {
            store: Mutex::new(Store::default()),
            budget,
        }
    }

    /// The rows `selection` selects in `source`, and the query's fingerprint.
    ///
    /// # Errors
    ///
    /// Returns an error when the query does not compile or evaluate.
    pub(crate) fn rows(
        &self,
        _structure: StructureId,
        source: &MolecularSource,
        selection: &Selection,
    ) -> Result<(Arc<AtomSelection>, u64), Error> {
        let query = selection.compiled()?;
        let fingerprint = query.fingerprint().get();
        let key = (source.identity(), source.coordinate_revision(), fingerprint);
        if let Ok(mut store) = self.store.lock()
            && let Some(rows) = store.touch(key)
        {
            return Ok((rows, fingerprint));
        }
        let rows = Arc::new(source.select_compiled(&query)?);
        // A poisoned lock only loses reuse; the evaluated rows are still right.
        if let Ok(mut store) = self.store.lock() {
            store.insert(key, Arc::clone(&rows), self.budget);
        }
        Ok((rows, fingerprint))
    }

    /// Forgets every evaluation over the source `identity`, for a molecule no
    /// structure of the scene is bound to any more.
    pub(crate) fn forget(&self, identity: u64) {
        if let Ok(mut store) = self.store.lock() {
            let doomed: Vec<RowKey> = store
                .entries
                .keys()
                .filter(|key| key.0 == identity)
                .copied()
                .collect();
            for key in doomed {
                if let Some(entry) = store.entries.remove(&key) {
                    let _ = store.order.remove(&entry.stamp);
                    store.bytes -= entry.cost;
                }
            }
        }
    }

    /// Bytes currently charged.
    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        match self.store.lock() {
            Ok(store) => store.bytes,
            Err(_) => 0,
        }
    }

    /// Number of remembered evaluations.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        match self.store.lock() {
            Ok(store) => store.entries.len(),
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
#[path = "selection_rows_tests.rs"]
mod tests;
