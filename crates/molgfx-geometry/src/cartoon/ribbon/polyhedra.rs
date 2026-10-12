//! Distance-defined atom triangles over borrowed, interpolated coordinates.

use super::{RibbonDeformation, RibbonMesh, RibbonVertex};
use crate::PackingError;
use molgfx_core::{AtomSelection, EntityId, EntityKind, PlacedStructure};
use molgfx_math::{Aabb, Rgba8, Vec3};

/// Limits both the candidate search and resident triangle stream.
const MAX_TRIANGLES: usize = 500_000;
const MAX_CANDIDATE_TESTS: usize = 8_000_000;

#[cfg(test)]
#[path = "polyhedra_tests.rs"]
mod tests;

impl RibbonMesh {
    /// Whether an interval candidate has a non-collapsed face at this exact pose.
    /// Uses the same central distances and area threshold as GPU deformation.
    #[must_use]
    pub fn has_sampled_polyhedra(&self, placed: &PlacedStructure, cutoff: f32) -> bool {
        if !cutoff.is_finite() || cutoff <= 0.0 || !(cutoff * cutoff).is_finite() {
            return false;
        }
        self.deformations.as_chunks::<3>().0.iter().any(|face| {
            if face[0].parameter[3].to_bits() != 7 {
                return false;
            }
            let [Some(a), Some(b), Some(c), Some(center)] =
                face[0].controls.map(|row| placed.sampled_position(row))
            else {
                return false;
            };
            let limit = cutoff * cutoff;
            (a - center).length_squared() <= limit
                && (b - center).length_squared() <= limit
                && (c - center).length_squared() <= limit
                && (b - a).cross(c - a).length_squared() > 1e-12
        })
    }
    /// Connects triplets of neighbors around every selected central atom.
    /// Candidate bounds span the resident trajectory interval; the GPU tests
    /// the central atom's current distances and gathers the face corners.
    ///
    /// # Errors
    /// Refuses invalid cutoffs, hierarchy failures or exhausted geometry budgets.
    pub fn generate_polyhedra(
        &mut self,
        placed: &PlacedStructure,
        selection: &AtomSelection,
        cutoff: f32,
        color: Rgba8,
    ) -> Result<(), PackingError> {
        if !cutoff.is_finite() || cutoff <= 0.0 || !(cutoff * cutoff).is_finite() {
            return Err(PackingError::GeometryBudget {
                resource: "positive finite polyhedra cutoff",
            });
        }
        let hierarchy = placed.render_bvh().map_err(PackingError::Hierarchy)?;
        let coordinates = placed.source.coordinates();
        let atoms = &placed.source.topology().atoms;
        let cutoff_squared = cutoff * cutoff;
        let mut stack = Vec::new();
        let mut candidates = Vec::new();
        let mut tests = 0usize;
        self.clear();
        for (center, point) in coordinates.iter().enumerate() {
            let center = u32::try_from(center).map_err(|_| PackingError::GeometryBudget {
                resource: "polyhedra atom rows",
            })?;
            if !selection.contains(center) {
                continue;
            }
            let bounds = atom_bounds(placed, center, *point);
            let margin = Vec3::splat(cutoff);
            hierarchy.aabb_candidates(
                Aabb::new(bounds.min - margin, bounds.max + margin),
                &mut stack,
                &mut candidates,
            );
            candidates.retain(|&row| {
                row != center
                    && molframe::altloc_compatible(
                        atoms[center as usize].alt_id,
                        atoms[row as usize].alt_id,
                    )
                    && box_distance_squared(
                        bounds,
                        atom_bounds(placed, row, coordinates[row as usize]),
                    ) <= cutoff_squared
            });
            candidates.sort_unstable();
            candidates.dedup();
            for (index, &first) in candidates.iter().enumerate() {
                for (offset, &second) in candidates[index + 1..].iter().enumerate() {
                    for &third in &candidates[index + offset + 2..] {
                        tests += 1;
                        if tests > MAX_CANDIDATE_TESTS || self.indices.len() / 3 >= MAX_TRIANGLES {
                            self.clear();
                            return Err(PackingError::GeometryBudget {
                                resource: "polyhedra face candidates",
                            });
                        }
                        let labels = [first, second, third].map(|row| atoms[row as usize].alt_id);
                        if !molframe::altloc_compatible(labels[0], labels[1])
                            || !molframe::altloc_compatible(labels[0], labels[2])
                            || !molframe::altloc_compatible(labels[1], labels[2])
                        {
                            continue;
                        }
                        if placed.trajectory().is_none() {
                            let [a, b, c] = [first, second, third]
                                .map(|row| Vec3::from(coordinates[row as usize]));
                            if (b - a).cross(c - a).length_squared() <= 1e-12 {
                                continue;
                            }
                        }
                        self.append_atom_triangle(
                            center,
                            [first, second, third],
                            coordinates,
                            cutoff,
                            color,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn append_atom_triangle(
        &mut self,
        center: u32,
        rows: [u32; 3],
        coordinates: &[[f32; 3]],
        cutoff: f32,
        color: Rgba8,
    ) -> Result<(), PackingError> {
        let [a, b, c] = rows.map(|row| Vec3::from(coordinates[row as usize]));
        let normal = match (b - a).cross(c - a).try_normalize() {
            Some(normal) => normal,
            None => Vec3::Z,
        };
        for (row, corner) in rows.into_iter().zip([0.0, 1.0, 2.0]) {
            let index =
                u32::try_from(self.vertices.len()).map_err(|_| PackingError::GeometryBudget {
                    resource: "polyhedra vertices",
                })?;
            self.vertices.push(RibbonVertex {
                position: coordinates[row as usize],
                normal: normal.to_array(),
                entity_id: EntityId::pack(EntityKind::Atom, u64::from(center))?.0,
                color,
            });
            self.deformations.push(RibbonDeformation {
                controls: [rows[0], rows[1], rows[2], center],
                parameter: [corner, cutoff, 0.0, f32::from_bits(7)],
            });
            self.indices.push(index);
        }
        Ok(())
    }
}

fn atom_bounds(placed: &PlacedStructure, row: u32, base: [f32; 3]) -> Aabb {
    match placed.trajectory() {
        Some(segment) => Aabb::from_points([
            Vec3::from(segment.start().positions()[row as usize]),
            Vec3::from(segment.end().positions()[row as usize]),
        ]),
        None => Aabb::from_points([Vec3::from(base)]),
    }
}

fn box_distance_squared(a: Aabb, b: Aabb) -> f32 {
    let separation = (a.min - b.max).max(b.min - a.max).max(Vec3::ZERO);
    separation.length_squared()
}
