//! The structural hierarchy as offset arrays.
//!
//! Residue, chain and model boundaries are stored as start offsets into the
//! next level down: residue `i` owns atoms `[residue_atom_start[i],
//! residue_atom_start[i+1])`, and likewise upward. Membership queries are a
//! binary search, `O(log n)`; range queries are `O(1)`.

use std::ops::Range;

#[cfg(test)]
#[path = "hierarchy_tests.rs"]
mod tests;

/// Offset-array form of the model → chain → residue → atom hierarchy.
#[derive(Clone, Debug, Default)]
pub struct Hierarchy {
    /// `residue_atom_start[i]..residue_atom_start[i+1]` are residue `i`'s
    /// atoms; length is residue count + 1.
    pub residue_atom_start: Vec<u32>,
    /// Chain `i`'s residues, same convention.
    pub chain_residue_start: Vec<u32>,
    /// Model `i`'s chains, same convention.
    pub model_chain_start: Vec<u32>,
}

impl Hierarchy {
    /// Copies compact offsets from a provider-neutral source once per asset.
    #[must_use]
    pub fn from_source(source: &crate::MolecularSource) -> Self {
        let topology = source.topology();
        Self {
            residue_atom_start: topology.residue_atom_start.to_vec(),
            chain_residue_start: topology.chain_residue_start.to_vec(),
            model_chain_start: topology.model_chain_start.to_vec(),
        }
    }

    /// Builds the offset arrays from the parsed structure's topology tables.
    /// `O(residues + chains + models)`.
    #[must_use]
    pub fn from_structure(structure: &molframe::Structure) -> Self {
        let topology = &structure.engine().data().topology;

        let residue_count = topology.residues.len();
        let mut residue_atom_start = Vec::with_capacity(residue_count + 1);
        let mut end = 0u32;
        for i in 0..residue_count {
            let index = molframe::ResidueIndex::new(crate::column::saturating_u32(i));
            let range = match topology.residues.atoms(index) {
                Some(range) => range,
                None => end..end,
            };
            residue_atom_start.push(range.start);
            end = range.end;
        }
        residue_atom_start.push(end);

        let chain_count = topology.chains.len();
        let mut chain_residue_start = Vec::with_capacity(chain_count + 1);
        let mut end = 0u32;
        for i in 0..chain_count {
            let index = molframe::ChainIndex::new(crate::column::saturating_u32(i));
            let range = match topology.chains.residues(index) {
                Some(range) => range,
                None => end..end,
            };
            chain_residue_start.push(range.start);
            end = range.end;
        }
        chain_residue_start.push(end);

        let mut model_chain_start = Vec::new();
        let mut end = 0u32;
        for model in topology.models.iter() {
            let range = match topology.models.chains(model) {
                Some(range) => range,
                None => end..end,
            };
            model_chain_start.push(range.start);
            end = range.end;
        }
        model_chain_start.push(end);

        Self {
            residue_atom_start,
            chain_residue_start,
            model_chain_start,
        }
    }

    /// Number of residues.
    #[must_use]
    pub fn residue_count(&self) -> usize {
        self.residue_atom_start.len().saturating_sub(1)
    }

    /// Number of chains.
    #[must_use]
    pub fn chain_count(&self) -> usize {
        self.chain_residue_start.len().saturating_sub(1)
    }

    /// The atoms of residue `i`, or an empty range past the end.
    #[must_use]
    pub fn residue_atoms(&self, i: usize) -> Range<u32> {
        range_at(&self.residue_atom_start, i)
    }

    /// The residues of chain `i`, or an empty range past the end.
    #[must_use]
    pub fn chain_residues(&self, i: usize) -> Range<u32> {
        range_at(&self.chain_residue_start, i)
    }

    /// The residue containing an atom, `O(log residues)`.
    #[must_use]
    pub fn residue_of_atom(&self, atom: u32) -> Option<usize> {
        containing(&self.residue_atom_start, atom)
    }

    /// The chain containing a residue, `O(log chains)`.
    #[must_use]
    pub fn chain_of_residue(&self, residue: u32) -> Option<usize> {
        containing(&self.chain_residue_start, residue)
    }
}

fn range_at(starts: &[u32], i: usize) -> Range<u32> {
    match (starts.get(i), starts.get(i + 1)) {
        (Some(&s), Some(&e)) => s..e,
        _ => 0..0,
    }
}

/// Index of the interval containing `value` in a start-offset array.
fn containing(starts: &[u32], value: u32) -> Option<usize> {
    if starts.len() < 2 {
        return None;
    }
    let last = *starts.last()?;
    if value >= last {
        return None;
    }
    // partition_point finds the first start beyond value; the interval is
    // the one before it.
    let i = starts.partition_point(|&s| s <= value);
    i.checked_sub(1)
}
