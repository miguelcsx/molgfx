//! Packing one shared record set from a placed structure.

use crate::error::RenderError;
use crate::scene_gpu::record_cache::{RecordPrepare, RecordScratch};
use molgfx_core::RepresentationKind;
use molgfx_gpu::Device;

/// Packs one record set's atoms, bonds and compaction map into scratch.
pub(super) fn pack_records<D: Device>(
    input: &RecordPrepare<'_, D>,
    scratch: &mut RecordScratch<'_>,
) -> Result<(), RenderError> {
    if input.representation.kind == RepresentationKind::Beads {
        molgfx_geometry::pack_residue_beads(
            &input.placed.atoms,
            &input.placed.hierarchy,
            input.placed.secondary_structure.values(),
            input.color_property,
            input.representation,
            input.selection,
            scratch.atoms,
        )?;
        molgfx_geometry::build_compaction_map(
            scratch.atoms,
            input.placed.atoms.len(),
            scratch.compaction,
        )?;
        scratch.bonds.clear();
        return Ok(());
    }
    molgfx_geometry::pack_atoms_with_properties(
        &input.placed.atoms,
        &input.placed.hierarchy,
        input.placed.secondary_structure.values(),
        molgfx_geometry::PropertyColumns {
            color: input.color_property,
            appearance: input.appearance_property,
            overlay: None,
        },
        input.representation,
        input.selection,
        scratch.atoms,
    )?;
    molgfx_geometry::build_compaction_map(
        scratch.atoms,
        input.placed.atoms.len(),
        scratch.compaction,
    )?;
    molgfx_geometry::pack_bonds(
        input.placed,
        input.representation,
        scratch.compaction,
        scratch.bonds,
    )?;
    Ok(())
}
