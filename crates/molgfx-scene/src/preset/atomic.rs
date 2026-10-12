//! Cold-path atomic presentation partitioning in O(atoms + bonds).
//!
//! Connectivity is borrowed from the molecular source. This chooses a drawable
//! shape without assigning chemical charge: an isolated atom needs a sphere,
//! while a covalently connected component can communicate its bonds.

use crate::{Error, Selection};
use molgfx_core::MolecularSource;
use std::fmt::Write as _;

/// Disjoint drawable targets partitioning one atomic selection.
#[derive(Clone, PartialEq, Debug)]
pub struct AtomicTargets {
    /// Atoms joined by an internal covalent edge in the requested selection.
    pub connected: Option<Selection>,
    /// Atoms without such an edge, including unresolved monatomic components.
    pub isolated: Option<Selection>,
}

/// Partitions an atomic target without inferring ions or inventing connectivity.
///
/// Metal coordination does not turn an isolated component into a covalent one.
/// Atom-row selectors remain portable over the same immutable source topology.
///
/// # Errors
/// Returns an error for an invalid target or source exceeding addressable rows.
pub fn atomic_targets(
    source: &MolecularSource,
    target: &Selection,
) -> Result<AtomicTargets, Error> {
    let count = u32::try_from(source.coordinates().len())
        .map_err(|_| Error::InvalidSpec("atomic source exceeds addressable rows".to_owned()))?;
    let selected = source.select_compiled(&*target.compiled()?)?;
    let mut connected = vec![false; count as usize];
    for bond in source.topology().bonds.iter() {
        if !bond.metal && selected.contains(bond.atoms[0]) && selected.contains(bond.atoms[1]) {
            for row in bond.atoms {
                if let Some(slot) = connected.get_mut(row as usize) {
                    *slot = true;
                }
            }
        }
    }
    let mut bonded_rows = String::new();
    let mut isolated_rows = String::new();
    for row in 0..count {
        if !selected.contains(row) {
            continue;
        }
        let output = if connected[row as usize] {
            &mut bonded_rows
        } else {
            &mut isolated_rows
        };
        write!(output, " {row}").map_err(|error| Error::InvalidSpec(error.to_string()))?;
    }
    Ok(AtomicTargets {
        connected: rows(bonded_rows),
        isolated: rows(isolated_rows),
    })
}

fn rows(mut indices: String) -> Option<Selection> {
    if indices.is_empty() {
        None
    } else {
        indices.insert_str(0, "index");
        Some(Selection::from(indices))
    }
}
