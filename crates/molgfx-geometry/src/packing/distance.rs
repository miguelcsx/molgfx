//! Geometric pair candidates; these never alter molecular covalent topology.

use crate::PackingError;
use molgfx_core::{BondGpu, EntityId, EntityKind, PlacedStructure, Representation};
use molgfx_math::{Aabb, Vec3};

const MAX_DISTANCE_PAIRS: usize = 2_000_000;

pub(super) fn pack_distance_bonds(
    placed: &PlacedStructure,
    representation: &Representation,
    compaction: &[u32],
    out: &mut Vec<BondGpu>,
) -> Result<(), PackingError> {
    let cutoff = representation.params.distance_cutoff;
    if !cutoff.is_finite() || cutoff <= 0.0 {
        return Err(PackingError::GeometryBudget {
            resource: "positive finite distance cutoff",
        });
    }
    let hierarchy = placed.render_bvh().map_err(PackingError::Hierarchy)?;
    let positions = placed.source.coordinates();
    let mut traversal = Vec::new();
    let mut candidates = Vec::new();
    let limit = cutoff * cutoff;
    for (row, &compact) in compaction.iter().enumerate() {
        if compact == u32::MAX {
            continue;
        }
        let first = u32::try_from(row).map_err(|_| PackingError::GeometryBudget {
            resource: "distance pair atom rows",
        })?;
        let bounds = bounds(placed, row, positions[row]);
        let margin = Vec3::splat(cutoff);
        hierarchy.aabb_candidates(
            Aabb::new(bounds.min - margin, bounds.max + margin),
            &mut traversal,
            &mut candidates,
        );
        candidates.sort_unstable();
        candidates.dedup();
        for &second in &candidates {
            if second <= first || compaction[second as usize] == u32::MAX {
                continue;
            }
            let atoms = &placed.source.topology().atoms;
            if !molframe::altloc_compatible(atoms[row].alt_id, atoms[second as usize].alt_id) {
                continue;
            }
            let other = bounds_for_row(placed, second as usize, positions);
            let separation = (bounds.min - other.max)
                .max(other.min - bounds.max)
                .max(Vec3::ZERO);
            if separation.length_squared() > limit {
                continue;
            }
            if out.len() >= MAX_DISTANCE_PAIRS {
                out.clear();
                return Err(PackingError::GeometryBudget {
                    resource: "distance bond pairs",
                });
            }
            // A geometric edge has no chemical bond row. Picking identifies
            // its first source atom instead of inventing a covalent identity.
            out.push(BondGpu::new(
                compact,
                compaction[second as usize],
                representation.params.bond_radius,
                false,
                EntityId::pack(EntityKind::Atom, u64::from(first))?,
            ));
        }
    }
    Ok(())
}

fn bounds_for_row(placed: &PlacedStructure, row: usize, positions: &[[f32; 3]]) -> Aabb {
    bounds(placed, row, positions[row])
}

fn bounds(placed: &PlacedStructure, row: usize, base: [f32; 3]) -> Aabb {
    match placed.trajectory() {
        Some(segment) => Aabb::from_points([
            Vec3::from(segment.start().positions()[row]),
            Vec3::from(segment.end().positions()[row]),
        ]),
        None => Aabb::from_points([Vec3::from(base)]),
    }
}
