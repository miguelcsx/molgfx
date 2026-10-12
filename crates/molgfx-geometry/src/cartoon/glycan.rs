//! Ring-oriented ribbons over upstream chemically classified glycosidic links.

use super::traces::{PolymerTraces, TraceRange};
use crate::PackingError;
use molgfx_core::AtomSelection;
use molgfx_math::Vec3;

#[cfg(test)]
#[path = "glycan_tests.rs"]
mod tests;

struct Sugar {
    centre: Vec3,
    normal: Vec3,
    entity: u32,
}

/// Extracts maximal unbranched runs through an upstream saccharide inventory.
///
/// Complete selected rings supply their chemically ordered plane. Only links
/// classified by molecular chemistry connect them; centre proximity never
/// creates a glycosidic bond. Each edge appears once, including branch arms
/// and closed cycles. Incomplete rings produce no fabricated geometry.
///
/// # Errors
/// Returns a topology error when molecular chemistry cannot resolve the source.
pub fn extract_glycosidic_traces(
    structure: &molframe::Structure,
    selection: &AtomSelection,
    output: &mut PolymerTraces,
) -> Result<(), PackingError> {
    let _ = extract_with_rings(structure, selection, output)?;
    Ok(())
}

pub(super) fn extract_with_rings(
    structure: &molframe::Structure,
    selection: &AtomSelection,
    output: &mut PolymerTraces,
) -> Result<Vec<[u32; 7]>, PackingError> {
    let report = molframe::chemistry::carbohydrates(
        structure.engine(),
        None,
        molframe::chemistry::CarbohydrateOptions::default(),
    )
    .map_err(|_| PackingError::Topology {
        operation: "saccharide ring and linkage inventory",
    })?;
    let sugars = report
        .monosaccharides
        .iter()
        .map(|sugar| {
            if !sugar
                .ring_atoms
                .iter()
                .all(|atom| selection.contains(atom.get()))
            {
                return None;
            }
            let geometry = sugar.geometry?;
            Some(Sugar {
                centre: Vec3::from(geometry.center),
                normal: Vec3::from(geometry.normal),
                entity: sugar.ring_atoms.first()?.get(),
            })
        })
        .collect::<Vec<_>>();
    let mut links = vec![Vec::new(); sugars.len()];
    let mut edges = Vec::with_capacity(report.links.len());
    for link in &report.links {
        let Some(acceptor) = link.acceptor else {
            continue;
        };
        if !selection.contains(link.donor_atom.get())
            || !selection.contains(link.acceptor_atom.get())
            || sugars.get(link.donor).is_none_or(Option::is_none)
            || sugars.get(acceptor).is_none_or(Option::is_none)
            || link.donor == acceptor
        {
            continue;
        }
        edges.push((link.donor.min(acceptor), link.donor.max(acceptor)));
    }
    edges.sort_unstable();
    edges.dedup();
    for (edge, &(first, second)) in edges.iter().enumerate() {
        links[first].push((second, edge));
        links[second].push((first, edge));
    }
    build_runs(&sugars, &links, edges.len(), output);
    let mut rings = Vec::with_capacity(sugars.len());
    for (sugar, selected) in report.monosaccharides.iter().zip(&sugars) {
        if selected.is_none() {
            continue;
        }
        let mut rows = [u32::MAX; 7];
        rows[0] =
            u32::try_from(sugar.ring_atoms.len()).map_err(|_| PackingError::IndexOverflow {
                resource: "saccharide ring size",
                index: sugar.ring_atoms.len() as u64,
            })?;
        for (row, atom) in rows[1..].iter_mut().zip(&sugar.ring_atoms) {
            *row = atom.get();
        }
        rings.push(rows);
    }
    Ok(rings)
}

fn build_runs(
    sugars: &[Option<Sugar>],
    links: &[Vec<(usize, usize)>],
    edge_count: usize,
    output: &mut PolymerTraces,
) {
    let mut points = Vec::with_capacity(edge_count * 2);
    let mut entities = Vec::with_capacity(edge_count * 2);
    let mut normals = Vec::with_capacity(edge_count * 2);
    let mut ranges = Vec::new();
    let mut visited = vec![false; edge_count];
    // Branches and endpoints delimit runs; remaining edges belong to cycles.
    for cycles in [false, true] {
        for (start, neighbours) in links.iter().enumerate() {
            if !cycles && neighbours.len() == 2 {
                continue;
            }
            for &(next, edge) in neighbours {
                if visited[edge] {
                    continue;
                }
                let begin = points.len();
                push_sugar(sugars, start, &mut points, &mut entities, &mut normals);
                let mut current = next;
                let mut entering = edge;
                loop {
                    visited[entering] = true;
                    push_sugar(sugars, current, &mut points, &mut entities, &mut normals);
                    let neighbours = &links[current];
                    if neighbours.len() != 2 {
                        break;
                    }
                    let Some(&(next, edge)) = neighbours.iter().find(|(_, edge)| !visited[*edge])
                    else {
                        break;
                    };
                    current = next;
                    entering = edge;
                }
                ranges.push(TraceRange {
                    chain: 0,
                    points: begin..points.len(),
                });
            }
        }
    }
    output.rebuild_from(points, entities, normals, ranges);
}

fn push_sugar(
    sugars: &[Option<Sugar>],
    index: usize,
    points: &mut Vec<Vec3>,
    entities: &mut Vec<u32>,
    normals: &mut Vec<Vec3>,
) {
    let Some(Some(sugar)) = sugars.get(index) else {
        return;
    };
    points.push(sugar.centre);
    entities.push(sugar.entity);
    normals.push(sugar.normal);
}
