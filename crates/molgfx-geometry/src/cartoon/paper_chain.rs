//! Topology-backed ring queries and continuous puckering colours.
//!
//! Cycle matching is delegated to `MolFrame`. With maximum ring size k and
//! maximum degree d its search is O(atoms × d^k), performed at topology changes.

use crate::{PackingError, RibbonMesh, RibbonVertex};
use molgfx_core::AtomSelection;
use molgfx_math::{Rgba8, Vec3};
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "paper_chain_tests.rs"]
mod tests;

/// Appends selected molecular ring bipyramids with total height in ångström.
/// Molecular bonds, rather than residue names, determine the rings.
///
/// # Errors
/// Returns an error for unavailable topology, invalid controls or GPU limits.
pub fn append_paper_chain(
    structure: &molframe::Structure,
    selection: &AtomSelection,
    height: f32,
    opacity: u8,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) -> Result<(), PackingError> {
    let mut mesh = RibbonMesh::default();
    mesh.generate_ring_bipyramids(structure, selection, height, 6, opacity)?;
    let offset = u32::try_from(vertices.len()).map_err(|_| PackingError::IndexOverflow {
        resource: "ring vertices",
        index: vertices.len() as u64,
    })?;
    for index in &mesh.indices {
        offset
            .checked_add(*index)
            .ok_or(PackingError::IndexOverflow {
                resource: "ring indices",
                index: u64::from(offset) + u64::from(*index),
            })?;
    }
    indices.extend(mesh.indices.iter().map(|index| offset + index));
    vertices.extend_from_slice(&mesh.vertices);
    Ok(())
}

pub(super) fn selected_rings(
    structure: &molframe::Structure,
    selection: &AtomSelection,
    maximum: u8,
) -> Result<BTreeSet<Vec<u32>>, PackingError> {
    check_search_budget(structure, maximum)?;
    let mut rings = BTreeSet::new();
    for size in 3..=maximum {
        let query = format!("*1{}~1", "~*".repeat(usize::from(size - 1)));
        let pattern = molframe::chemistry::SmartsPattern::parse(&query).map_err(|_| {
            PackingError::Topology {
                operation: "cycle pattern",
            }
        })?;
        let matches = pattern
            .find_structure_matches(structure.engine())
            .map_err(|_| PackingError::Topology {
                operation: "molecular ring connectivity",
            })?;
        for matched in matches {
            let rows: Vec<u32> = matched
                .atom_indices
                .iter()
                .copied()
                .map(|row| {
                    u32::try_from(row).map_err(|_| PackingError::IndexOverflow {
                        resource: "ring atom rows",
                        index: row as u64,
                    })
                })
                .collect::<Result<_, _>>()?;
            if rows.iter().any(|row| !selection.contains(*row)) {
                continue;
            }
            let Some((start, _)) = rows.iter().enumerate().min_by_key(|(_, row)| *row) else {
                continue;
            };
            let forward: Vec<u32> = (0..rows.len())
                .map(|i| rows[(start + i) % rows.len()])
                .collect();
            let reverse: Vec<u32> = (0..rows.len())
                .map(|i| rows[(start + rows.len() - i) % rows.len()])
                .collect();
            rings.insert(forward.min(reverse));
            if rings.len() > 100_000 {
                return Err(PackingError::GeometryBudget {
                    resource: "molecular rings",
                });
            }
        }
    }
    Ok(rings)
}

// Count walks without visited-node pruning. This bounds the matcher's search
// before allocating matches, at O(maximum × bonds) cost and O(atoms) storage.
fn check_search_budget(structure: &molframe::Structure, maximum: u8) -> Result<(), PackingError> {
    const MAX_SEARCH_STATES: u64 = 200_000_000;
    let source = structure.engine();
    let adjacency = source.data().bonds.adjacency(source.atom_count());
    let mut previous = vec![1u64; source.atom_count() as usize];
    let mut current = vec![0u64; previous.len()];
    let mut total = 0u64;
    for depth in 1..maximum {
        for (row, value) in current.iter_mut().enumerate() {
            let row = u32::try_from(row).map_err(|_| PackingError::GeometryBudget {
                resource: "ring atom rows",
            })?;
            *value = adjacency
                .neighbours(molframe::AtomIndex::new(row))
                .iter()
                .fold(0u64, |sum, neighbor| {
                    sum.saturating_add(previous[neighbor.as_usize()])
                });
        }
        let visits = current
            .iter()
            .fold(0u64, |sum, count| sum.saturating_add(*count));
        total = total.saturating_add(visits.saturating_mul(u64::from(maximum - depth)));
        if total > MAX_SEARCH_STATES {
            return Err(PackingError::GeometryBudget {
                resource: "complete molecular ring search",
            });
        }
        std::mem::swap(&mut previous, &mut current);
    }
    Ok(())
}

const PUCKER_STOPS: [(u16, [u8; 3]); 6] = [
    (0, [220, 62, 55]),
    (19, [240, 178, 48]),
    (48, [74, 176, 96]),
    (78, [54, 190, 194]),
    (102, [65, 105, 225]),
    (130, [184, 76, 190]),
];

pub(super) fn append_palette(values: &mut Vec<f32>, opacity: u8) {
    for (amplitude, [r, g, b]) in PUCKER_STOPS {
        values.push(f32::from(amplitude) / 100.0);
        values.push(f32::from_bits(u32::from_le_bytes([r, g, b, opacity])));
    }
}

pub(super) fn pucker_amplitude(points: &[Vec3]) -> f32 {
    let Ok(count) = u8::try_from(points.len()) else {
        return 0.0;
    };
    if count < 3 {
        return 0.0;
    }
    let center = points.iter().copied().sum::<Vec3>() / f32::from(count);
    let mut sine = Vec3::ZERO;
    let mut cosine = Vec3::ZERO;
    for (index, point) in points.iter().enumerate() {
        let Ok(index) = u8::try_from(index) else {
            return 0.0;
        };
        let (s, c) = (std::f32::consts::TAU * f32::from(index) / f32::from(count)).sin_cos();
        sine += (*point - center) * s;
        cosine += (*point - center) * c;
    }
    let Some(normal) = sine.cross(cosine).try_normalize() else {
        return 0.0;
    };
    points
        .iter()
        .map(|point| (*point - center).dot(normal).powi(2))
        .sum::<f32>()
        .sqrt()
}

pub(super) fn pucker_color(amplitude: f32, opacity: u8) -> Rgba8 {
    let scaled = centi_angstrom(amplitude);
    let mut color = PUCKER_STOPS[PUCKER_STOPS.len() - 1].1;
    for window in PUCKER_STOPS.windows(2) {
        let [(low, start), (high, end)] = [window[0], window[1]];
        if scaled >= high {
            continue;
        }
        color = [
            mix_channel(start[0], end[0], scaled - low, high - low),
            mix_channel(start[1], end[1], scaled - low, high - low),
            mix_channel(start[2], end[2], scaled - low, high - low),
        ];
        break;
    }
    Rgba8::new(color[0], color[1], color[2], opacity)
}

/// Blends one channel between two stops without leaving integer arithmetic.
fn mix_channel(start: u8, end: u8, amount: u16, span: u16) -> u8 {
    let span = u32::from(span.max(1));
    let amount = u32::from(amount).min(span);
    let low = u32::from(start.min(end));
    let step = (u32::from(start.max(end)) - low) * amount / span;
    let value = if start <= end {
        low + step
    } else {
        u32::from(start) - step
    };
    u8::try_from(value)
        .into_iter()
        .fold(u8::MAX, |_, byte| byte)
}

/// Amplitude in hundredths of an Angstrom, saturating rather than wrapping.
///
/// Truncation rather than rounding: this is a magnitude, and rounding up would
/// report an amplitude the source never reached.
fn centi_angstrom(amplitude: f32) -> u16 {
    molgfx_math::truncate_u16(amplitude * 100.0)
}
