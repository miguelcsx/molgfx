//! Selected canonical pairing supplied by `MolFrame`.

use crate::RenderError;
use molgfx_core::AtomSelection;

pub(super) fn selected_pairs(
    structure: &molframe::Structure,
    selection: &AtomSelection,
) -> Result<AtomSelection, RenderError> {
    molgfx_geometry::paired_base_selection(structure, selection)
        .map_err(RenderError::BasePairGeometry)
}
