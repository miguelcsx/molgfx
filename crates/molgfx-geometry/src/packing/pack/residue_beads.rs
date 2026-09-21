//! Compact one-record-per-residue packing.

use super::representation_color;
use crate::PackingError;
use molgfx_core::{
    AtomGpu, AtomProperty, AtomSelection, AtomTable, EntityId, EntityKind, Hierarchy,
    Representation, SecondaryStructure,
};
use molgfx_math::{Rgba8, Vec3};

/// Packs one bead per residue, enclosing all selected atoms in that residue.
///
/// # Errors
///
/// Returns [`PackingError`] when a residue source row cannot be encoded.
pub fn pack_residue_beads(
    table: &AtomTable,
    hierarchy: &Hierarchy,
    secondary_structure: &[SecondaryStructure],
    property: Option<&AtomProperty>,
    representation: &Representation,
    selection: &AtomSelection,
    out: &mut Vec<AtomGpu>,
) -> Result<(), PackingError> {
    out.clear();
    let coords = table.coords().slice();
    let radii = table.radius().values();
    let colors = table.color().values();
    let elements = table.element().values();
    let flags = table.flags().values();
    let semantics = table.semantic().values();
    let residues = table.residue().values();
    let scale = representation.params.radius_scale.max(0.0);
    for residue_index in 0..hierarchy.residue_count() {
        let atoms = hierarchy.residue_atoms(residue_index);
        let Some(first_row) = atoms.clone().find(|row| selection.contains(*row)) else {
            continue;
        };
        let first = first_row as usize;
        let centre = coords
            .get(first)
            .map_or(Vec3::ZERO, |point| Vec3::from_array(*point));
        let radius = atoms
            .filter(|row| selection.contains(*row))
            .map(|row| {
                let index = row as usize;
                let point = coords
                    .get(index)
                    .map_or(centre, |value| Vec3::from_array(*value));
                let extent = radii
                    .get(index)
                    .copied()
                    .into_iter()
                    .fold(0.0, |_, value| value);
                centre.distance(point) + extent
            })
            .fold(0.0_f32, f32::max);
        let mut color = representation_color(
            representation.color,
            colors
                .get(first)
                .copied()
                .into_iter()
                .fold(Rgba8::opaque(255, 255, 255), |_, value| value),
            Some((hierarchy, secondary_structure)),
            property,
            residues.get(first).copied(),
            first,
        );
        color.a = representation.material.opacity_unorm8();
        let source = u64::try_from(first).map_err(|_| PackingError::IndexOverflow {
            resource: "residue bead source",
            index: u64::MAX,
        })?;
        out.push(AtomGpu {
            radius: radius * scale,
            color,
            element: elements
                .get(first)
                .copied()
                .into_iter()
                .fold(0, |_, value| value),
            flags: flags
                .get(first)
                .copied()
                .into_iter()
                .fold(molgfx_core::AtomFlags(0), |_, value| value),
            entity_id: EntityId::pack(EntityKind::Atom, source)?,
            semantic: semantics
                .get(first)
                .copied()
                .into_iter()
                .fold(0, |_, value| value),
        });
    }
    Ok(())
}
