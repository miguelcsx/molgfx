//! O(residue atoms) nucleotide ring faces with exact live atom corners.

use super::{RibbonDeformation, RibbonMesh, RibbonVertex};
use crate::PackingError;
use molgfx_core::{AtomSelection, EntityId, EntityKind};
use molgfx_math::{Rgba8, Vec3};

const RING: [&str; 5] = ["O4'", "C1'", "C2'", "C3'", "C4'"];
const BASE_HEXAGON: [&str; 6] = ["N1", "C2", "N3", "C4", "C5", "C6"];
const BASE_PENTAGON: [&str; 5] = ["N9", "C8", "N7", "C5", "C4"];

struct Face<const N: usize> {
    rows: [u32; N],
    points: [Vec3; N],
    conformer: molframe::AltId,
}

#[cfg(test)]
#[path = "sugar_tests.rs"]
mod tests;

impl RibbonMesh {
    pub(super) fn append_sugar_faces(
        &mut self,
        structure: &molframe::Structure,
        selection: &AtomSelection,
        color: Rgba8,
    ) -> Result<(), PackingError> {
        for residue in structure.residues() {
            let Some(sugar) = selected_sugar(residue, selection) else {
                continue;
            };
            self.append_face(&sugar, color)?;
            if let Some(face) =
                selected_face(residue, selection, BASE_HEXAGON, Some(sugar.conformer))
            {
                self.append_face(&face, color)?;
            }
            if let Some(face) =
                selected_face(residue, selection, BASE_PENTAGON, Some(sugar.conformer))
            {
                self.append_face(&face, color)?;
            }
        }
        Ok(())
    }

    fn append_face<const N: usize>(
        &mut self,
        face: &Face<N>,
        color: Rgba8,
    ) -> Result<(), PackingError> {
        let entity_id = EntityId::pack(EntityKind::Atom, u64::from(face.rows[0]))?.0;
        for edge in 1..N - 1 {
            let corners = [0, edge, edge + 1];
            let triangle = corners.map(|index| face.rows[index]);
            let points = corners.map(|index| face.points[index]);
            let Some(normal) = (points[1] - points[0])
                .cross(points[2] - points[0])
                .try_normalize()
            else {
                continue;
            };
            super::super::draw_limits::check_glyph(self.vertices.len(), self.indices.len())?;
            for (corner, position) in [0u8, 1, 2].into_iter().zip(points) {
                self.indices.push(super::super::draw_limits::vertex_index(
                    self.vertices.len(),
                )?);
                self.vertices.push(RibbonVertex {
                    position: position.to_array(),
                    normal: normal.to_array(),
                    entity_id,
                    color,
                });
                self.deformations.push(RibbonDeformation {
                    controls: [triangle[0], triangle[1], triangle[2], triangle[0]],
                    parameter: [f32::from(corner), 0.0, 0.0, f32::from_bits(10)],
                });
            }
        }
        Ok(())
    }
}

/// Whether a complete, positioned sugar face belongs to the selected rows.
#[must_use]
pub fn has_selected_sugar_faces(
    structure: &molframe::Structure,
    selection: &AtomSelection,
) -> bool {
    structure
        .residues()
        .into_iter()
        .any(|residue| selected_sugar(residue, selection).is_some())
}

fn selected_sugar(residue: molframe::ResidueRef<'_>, selection: &AtomSelection) -> Option<Face<5>> {
    selected_face(residue, selection, RING, None)
}

fn selected_face<const N: usize>(
    residue: molframe::ResidueRef<'_>,
    selection: &AtomSelection,
    names: [&str; N],
    preferred: Option<molframe::AltId>,
) -> Option<Face<N>> {
    let conformer = match preferred.or_else(|| {
        residue.atoms().find_map(|atom| {
            (selection.contains(atom.index().get())
                && names.iter().any(|name| atom.name() == Some(*name)))
            .then(|| atom.alt_id())
            .flatten()
            .filter(|alternate| *alternate != molframe::AltId::BLANK)
        })
    }) {
        Some(alternate) => alternate,
        None => molframe::AltId::BLANK,
    };
    let mut sugar = Face {
        rows: [0; N],
        points: [Vec3::ZERO; N],
        conformer,
    };
    for (index, name) in names.into_iter().enumerate() {
        let atom = residue.atoms().find(|atom| {
            let alternate = match atom.alt_id() {
                Some(value) => value,
                None => molframe::AltId::BLANK,
            };
            atom.name() == Some(name)
                && selection.contains(atom.index().get())
                && molframe::altloc_compatible(conformer, alternate)
        })?;
        sugar.rows[index] = atom.index().get();
        sugar.points[index] = Vec3::from(atom.position()?);
    }
    let points = sugar.points;
    (1..N - 1)
        .any(|edge| {
            (points[edge] - points[0])
                .cross(points[edge + 1] - points[0])
                .length_squared()
                > 1e-12
        })
        .then_some(sugar)
}
