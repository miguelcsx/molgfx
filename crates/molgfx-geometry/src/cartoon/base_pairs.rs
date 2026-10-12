//! Selected canonical pairing supplied by `MolFrame`, never inferred by the renderer.

use molgfx_core::AtomSelection;

/// Returns sugar anchors only when both residues of a supported pair are selected.
///
/// # Errors
/// Returns the upstream spatial error when pairing geometry cannot be evaluated.
pub fn paired_base_selection(
    structure: &molframe::Structure,
    selection: &AtomSelection,
) -> Result<AtomSelection, molframe::spatial::SpatialError> {
    let pairs = molframe::spatial::standard_base_pair_geometry(
        structure.engine(),
        molframe::chemistry::base_pair::WatsonCrickGeometry::default(),
        MAXIMUM_BASE_CONTACT_DISTANCE,
    )?;
    let mut rows = Vec::with_capacity(pairs.len() * 2);
    for pair in pairs {
        let (Some(left), Some(right)) = (
            structure.engine().data().residue(pair.first),
            structure.engine().data().residue(pair.second),
        ) else {
            continue;
        };
        if !left
            .atoms()
            .any(|atom| selection.contains(atom.index().get()))
            || !right
                .atoms()
                .any(|atom| selection.contains(atom.index().get()))
        {
            continue;
        }
        for residue in [left, right] {
            if let Some(sugar) = residue.atom("C1'") {
                rows.push(sugar.index().get());
            }
        }
    }
    rows.sort_unstable();
    rows.dedup();
    Ok(AtomSelection::Sparse(rows))
}

const MAXIMUM_BASE_CONTACT_DISTANCE: f32 = 3.6;
