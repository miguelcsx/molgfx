//! The columnar atom table.
//!
//! Coordinates stay borrowed from the parsed structure; everything else the
//! renderer needs per atom (element, residue, radius, color, flags, semantic
//! tag) is materialized once into owned columns at construction — a single
//! `O(atoms)` pass — because the parser stores those columns bit-packed and
//! the render path must read them densely without a decode branch per access.
//! Altlocs and insertion codes survive as ordinary rows; nothing collapses.

use crate::column::Column;
use crate::coord::CoordRef;
use crate::gpu_types::AtomFlags;
use crate::radii;
use pdviewx_math::Rgba8;

#[cfg(test)]
#[path = "atoms_tests.rs"]
mod tests;

/// Dense per-atom state: one borrowed coordinate column plus owned columns
/// in the structure's own atom order.
#[derive(Clone, Debug)]
pub struct AtomTable {
    len: u32,
    coords: CoordRef,
    element: Column<u16>,
    residue: Column<u32>,
    radius: Column<f32>,
    color: Column<Rgba8>,
    flags: Column<AtomFlags>,
    semantic: Column<u32>,
}

impl AtomTable {
    /// Materializes the table for one model of a parsed structure.
    /// `O(atoms)`, run once per placement, never per frame.
    #[must_use]
    pub fn from_structure(
        structure: &pdbiox::Structure,
        model: pdbiox::ModelIndex,
    ) -> Option<Self> {
        let coords = CoordRef::new(structure, model)?;
        let atom_count = coords.len();
        let data = structure.data();

        let mut element = vec![0u16; atom_count];
        let mut residue = vec![0u32; atom_count];
        let mut radius = vec![radii::vdw_radius(0); atom_count];
        let mut color = vec![radii::cpk_color(0); atom_count];

        for chunk in data.chunks.iter() {
            let range = chunk.atoms();
            for (local, global) in (0..chunk.len()).zip(range.clone()) {
                let slot = global as usize;
                if slot >= atom_count {
                    break;
                }
                if let Some(e) = chunk.element(local) {
                    let z = e.atomic_number();
                    element[slot] = u16::from(z);
                    radius[slot] = radii::vdw_radius(z);
                    color[slot] = radii::cpk_color(z);
                }
                if let Some(r) = chunk.residue(local, &data.topology.residues) {
                    residue[slot] = r.get();
                }
            }
        }

        Some(Self {
            len: crate::column::saturating_u32(atom_count),
            coords,
            element: Column::new(element),
            residue: Column::new(residue),
            radius: Column::new(radius),
            color: Column::new(color),
            flags: Column::new(vec![AtomFlags::VISIBLE; atom_count]),
            semantic: Column::new(vec![0u32; atom_count]),
        })
    }

    /// Number of atoms.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// True when the table has no atoms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The borrowed coordinate column.
    #[must_use]
    pub fn coords(&self) -> &CoordRef {
        &self.coords
    }

    /// Interned element ids (atomic numbers).
    #[must_use]
    pub fn element(&self) -> &Column<u16> {
        &self.element
    }

    /// Owning residue index per atom.
    #[must_use]
    pub fn residue(&self) -> &Column<u32> {
        &self.residue
    }

    /// Van der Waals radii, Ångström.
    #[must_use]
    pub fn radius(&self) -> &Column<f32> {
        &self.radius
    }

    /// Resolved display colors.
    #[must_use]
    pub fn color(&self) -> &Column<Rgba8> {
        &self.color
    }

    /// Mutable colors; bumps the color revision.
    pub fn color_mut(&mut self) -> &mut [Rgba8] {
        self.color.values_mut()
    }

    /// Visibility and emphasis flags.
    #[must_use]
    pub fn flags(&self) -> &Column<AtomFlags> {
        &self.flags
    }

    /// Mutable flags; bumps the flags revision.
    pub fn flags_mut(&mut self) -> &mut [AtomFlags] {
        self.flags.values_mut()
    }

    /// Packed semantic tags.
    #[must_use]
    pub fn semantic(&self) -> &Column<u32> {
        &self.semantic
    }

    /// Mutable semantic tags; bumps that column's revision.
    pub fn semantic_mut(&mut self) -> &mut [u32] {
        self.semantic.values_mut()
    }
}
