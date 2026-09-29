//! Packing one shared record set from a placed structure.

use crate::error::RenderError;
use crate::scene_gpu::record_cache::{RecordPrepare, RecordScratch};
use molgfx_core::RepresentationKind;
use molgfx_gpu::Device;

#[cfg(test)]
#[path = "record_pack_tests.rs"]
mod tests;

/// Packs one record set's atoms, bonds and compaction map into scratch.
pub(super) fn pack_records<D: Device>(
    input: &RecordPrepare<'_, D>,
    scratch: &mut RecordScratch<'_>,
) -> Result<(), RenderError> {
    if input.representation.kind == RepresentationKind::Beads {
        molgfx_geometry::pack_residue_beads(
            &input.placed.atoms,
            &input.placed.hierarchy,
            input.color,
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
    molgfx_geometry::pack_atoms(
        &input.placed.atoms,
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
    if input.representation.kind == RepresentationKind::Lines {
        hide_bonded_line_atoms(scratch.atoms, scratch.bonds);
    }
    Ok(())
}

/// Leaves a positive point-cull radius only on selected atoms without a bond.
///
/// Bond endpoints already index the compact atom table, so this is one linear
/// pass with no degree array or additional allocation.
fn hide_bonded_line_atoms(atoms: &mut [molgfx_core::AtomGpu], bonds: &[molgfx_core::BondGpu]) {
    for bond in bonds {
        atoms[bond.atom_a as usize].radius = 0.0;
        atoms[bond.atom_b as usize].radius = 0.0;
    }
}
