//! Residue coordinate recipes carried by the otherwise unused bond binding.
//! O(atoms) packing; the GPU gathers centroids and enclosures from borrowed XYZ.

use molgfx_core::{AtomGpu, AtomSelection, PlacedStructure};

pub(super) fn append_recipes(
    placed: &PlacedStructure,
    selection: &AtomSelection,
    beads: &[AtomGpu],
    compaction: &[u32],
    scale: f32,
    words: &mut Vec<u32>,
) -> Result<(), molgfx_geometry::PackingError> {
    words.clear();
    words.push(placed.atoms.len());
    words.extend_from_slice(compaction);
    let header = words.len();
    words.resize(header + beads.len() * 3, 0);
    let mut bead = 0;
    for residue in 0..placed.hierarchy.residue_count() {
        let members = placed.hierarchy.residue_atoms(residue);
        if !members.clone().any(|row| selection.contains(row)) {
            continue;
        }
        let start = words.len();
        for row in members.filter(|row| selection.contains(*row)) {
            words.push(row);
            words.push(placed.atoms.radius().values()[row as usize].to_bits());
        }
        words[header + bead * 3] = index(start)?;
        words[header + bead * 3 + 1] = index((words.len() - start) / 2)?;
        words[header + bead * 3 + 2] = scale.to_bits();
        bead += 1;
    }
    Ok(())
}

fn index(value: usize) -> Result<u32, molgfx_geometry::PackingError> {
    u32::try_from(value).map_err(|_| molgfx_geometry::PackingError::IndexOverflow {
        resource: "residue coordinate recipes",
        index: value as u64,
    })
}
