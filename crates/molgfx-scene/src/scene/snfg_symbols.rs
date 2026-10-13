//! SNFG symbols of a structure, built once per structure revision.
//!
//! Resolving a scene runs again for every structural edit, so the symbol set
//! (one chemistry pass over every monosaccharide) must not be rebuilt each
//! time. The set depends only on the molecule, so it is keyed by the source
//! identity and coordinate revision and shared by every representation, edit
//! and placed copy of that molecule. The cache is bounded and evicts the least
//! recently used entry.

use crate::error::Error;
use molgfx_core::MolecularSource;
use molgfx_geometry::SnfgSymbolSet;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

/// Distinct structure revisions remembered at once.
const CAPACITY: usize = 16;

type Entries = VecDeque<((u64, u64), Arc<SnfgSymbolSet>)>;

fn cache() -> &'static Mutex<Entries> {
    static CACHE: OnceLock<Mutex<Entries>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// The symbols of every complete monosaccharide ring of `source`.
///
/// # Errors
/// Returns an error when the source has no native molecular topology or its
/// chemistry cannot be resolved.
pub(crate) fn symbols_of(source: &MolecularSource) -> Result<Arc<SnfgSymbolSet>, Error> {
    let key = (source.identity(), source.coordinate_revision());
    if let Ok(mut entries) = cache().lock()
        && let Some(index) = entries.iter().position(|(known, _)| *known == key)
        && let Some(entry) = entries.remove(index)
    {
        let set = Arc::clone(&entry.1);
        entries.push_back(entry);
        return Ok(set);
    }
    let structure = source.molframe().ok_or_else(|| {
        Error::InvalidSpec("SNFG symbols need native molecular topology".to_owned())
    })?;
    let set = Arc::new(
        SnfgSymbolSet::build(structure, &molgfx_core::AtomSelection::All)
            .map_err(|error| Error::InvalidSpec(error.to_string()))?,
    );
    // A poisoned lock only loses reuse; the set itself is still right.
    if let Ok(mut entries) = cache().lock() {
        if entries.len() >= CAPACITY {
            let _ = entries.pop_front();
        }
        entries.push_back((key, Arc::clone(&set)));
    }
    Ok(set)
}

/// How many times the process built a set, for tests of the cache.
#[cfg(test)]
pub(crate) fn cached_len() -> usize {
    cache().lock().map_or(0, |entries| entries.len())
}
