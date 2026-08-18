//! Glycosidic traces: the ribbon path through a branched sugar tree.
//!
//! A protein backbone is a line, so its trace is one sweep down the residue
//! order. A glycan is a tree: one sugar can carry two or three others, and the
//! order residues appear in the file says nothing about which is bonded to
//! which. So the path has to be walked over the connectivity instead, and each
//! root-to-branch run becomes its own trace — which is exactly how a reader
//! follows a glycan by eye.
//!
//! Ring centres are used as guide points rather than any single atom: a sugar's
//! ring is its body, and its centre moves smoothly from one residue to the next
//! where an arbitrary ring atom would jitter.

use super::traces::{PolymerTraces, TraceRange};
use pdviewx_core::AtomSelection;
use pdviewx_math::Vec3;

#[cfg(test)]
#[path = "glycan_tests.rs"]
mod tests;

/// Pyranose and furanose ring atoms. A sugar ring is carbons plus its ring
/// oxygen, which distinguishes it from a nucleotide base — those carry ring
/// nitrogens — without consulting a component dictionary.
const RING_ATOMS: [&str; 7] = ["C1", "C2", "C3", "C4", "C5", "O5", "O4"];
/// Fewest ring atoms that still locate a sugar centre.
const MIN_RING_ATOMS: usize = 4;
/// Largest centre-to-centre distance treated as a glycosidic link, Ångström.
/// Two linked pyranose centres sit near 5.5 Å; beyond this they are separate
/// molecules that merely pack close.
const MAX_LINK_DISTANCE: f32 = 7.5;

/// One sugar reduced to the point a ribbon passes through.
struct Sugar {
    centre: Vec3,
    entity: u32,
}

/// Extracts one trace per linear run through the glycosidic tree.
///
/// Residues without a sugar ring are skipped rather than approximated, and a
/// tree with no links yields nothing rather than a ribbon through unrelated
/// residues.
pub fn extract_glycosidic_traces(
    structure: &pdbiox::Structure,
    selection: &AtomSelection,
    output: &mut PolymerTraces,
) {
    let sugars = collect_sugars(structure, selection);
    if sugars.len() < 2 {
        output.rebuild_from(Vec::new(), Vec::new(), Vec::new());
        return;
    }
    let links = link_graph(&sugars);

    let mut points = Vec::new();
    let mut entities = Vec::new();
    let mut ranges = Vec::new();
    let mut visited = vec![false; sugars.len()];

    // Walk from every sugar that is an end or a branch point: those are where a
    // run the eye can follow begins.
    for start in 0..sugars.len() {
        let degree = links.get(start).map_or(0, Vec::len);
        if degree == 2 {
            continue;
        }
        for &neighbour in links.get(start).into_iter().flatten() {
            if seen(&visited, neighbour) && degree != 1 {
                continue;
            }
            let begin = points.len();
            let mut current = start;
            let mut next = Some(neighbour);
            push_sugar(&sugars, current, &mut points, &mut entities, &mut visited);
            while let Some(index) = next {
                if seen(&visited, index) {
                    break;
                }
                push_sugar(&sugars, index, &mut points, &mut entities, &mut visited);
                // Continue only while the run stays unbranched.
                next = links
                    .get(index)
                    .filter(|neighbours| neighbours.len() == 2)
                    .and_then(|neighbours| {
                        neighbours
                            .iter()
                            .copied()
                            .find(|candidate| *candidate != current && !seen(&visited, *candidate))
                    });
                current = index;
            }
            if points.len() - begin >= 2 {
                ranges.push(TraceRange {
                    chain: 0,
                    points: begin..points.len(),
                });
            } else {
                points.truncate(begin);
                entities.truncate(begin);
            }
        }
    }
    output.rebuild_from(points, entities, ranges);
}

/// Whether a sugar has already been drawn. An index past the end counts as
/// visited, so a bad index ends the run rather than extending it.
fn seen(visited: &[bool], index: usize) -> bool {
    visited.get(index).is_none_or(|drawn| *drawn)
}

fn push_sugar(
    sugars: &[Sugar],
    index: usize,
    points: &mut Vec<Vec3>,
    entities: &mut Vec<u32>,
    visited: &mut [bool],
) {
    let Some(sugar) = sugars.get(index) else {
        return;
    };
    points.push(sugar.centre);
    entities.push(sugar.entity);
    if let Some(slot) = visited.get_mut(index) {
        *slot = true;
    }
}

/// Neighbour lists by centre proximity. Explicit glycosidic bonds are the
/// stronger signal, but many deposited entries omit them for branched glycans,
/// and a distance short enough to be a link is unambiguous at sugar scale.
fn link_graph(sugars: &[Sugar]) -> Vec<Vec<usize>> {
    let mut links = vec![Vec::new(); sugars.len()];
    for (first, left) in sugars.iter().enumerate() {
        for (second, right) in sugars.iter().enumerate().skip(first + 1) {
            if left.centre.distance(right.centre) > MAX_LINK_DISTANCE {
                continue;
            }
            if let Some(slot) = links.get_mut(first) {
                slot.push(second);
            }
            if let Some(slot) = links.get_mut(second) {
                slot.push(first);
            }
        }
    }
    links
}

fn collect_sugars(structure: &pdbiox::Structure, selection: &AtomSelection) -> Vec<Sugar> {
    let mut sugars = Vec::new();
    for chain in structure.data().chains() {
        for residue in chain.residues() {
            let mut ring = Vec::with_capacity(RING_ATOMS.len());
            let mut entity = None;
            for name in RING_ATOMS {
                let Some(atom) = residue.atom(name) else {
                    continue;
                };
                if !selection.contains(atom.index().get()) {
                    continue;
                }
                if let Some(position) = atom.position() {
                    ring.push(Vec3::from(position));
                    entity.get_or_insert(atom.index().get());
                }
            }
            // A ring oxygen is what separates a sugar from an open chain of
            // carbons that happens to share the naming.
            let closed = residue.atom("O5").is_some() || residue.atom("O4").is_some();
            if ring.len() < MIN_RING_ATOMS || !closed {
                continue;
            }
            let (sum, count) = ring.iter().fold((Vec3::ZERO, 0.0f32), |(sum, n), point| {
                (sum + *point, n + 1.0)
            });
            let Some(entity) = entity else {
                continue;
            };
            sugars.push(Sugar {
                centre: sum / count.max(1.0),
                entity,
            });
        }
    }
    sugars
}
