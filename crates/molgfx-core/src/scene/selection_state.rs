//! Stored selection rows and the query identity they were evaluated from.

use crate::handle::StructureHandle;

/// One stored mask: a global bitmap, or per-structure rows.
///
/// The optional fingerprint is what lets two selections that came from the same
/// normalized query be recognized as covering the same rows, so everything the
/// renderer derives from the molecule can be shared between them.
#[derive(Clone, Debug)]
pub(crate) struct StoredSelection {
    pub(crate) global: Option<crate::AtomSelection>,
    pub(crate) scoped: Vec<(StructureHandle, crate::AtomSelection)>,
    /// Normalized-query fingerprint this mask was evaluated from.
    ///
    /// Selections built by hand, by a provider, or by set algebra are not the
    /// image of any single query and carry `None`.
    pub(crate) query_fingerprint: Option<u64>,
}
