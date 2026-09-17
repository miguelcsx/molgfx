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
use molgfx_core::AtomSelection;
use molgfx_math::Vec3;

#[cfg(test)]
#[path = "glycan_tests.rs"]
mod tests;

/// Pyranose and furanose ring atoms. A sugar ring is carbons plus its ring
/// oxygen, which distinguishes it from a nucleotide base — those carry ring
/// nitrogens — without consulting a component dictionary.
const PYRANOSE_RING: [&str; 6] = ["O5", "C1", "C2", "C3", "C4", "C5"];
const FURANOSE_RING: [&str; 5] = ["O4", "C1", "C2", "C3", "C4"];
/// Fewest ring atoms that still locate a sugar centre.
const MIN_RING_ATOMS: usize = 4;
/// Largest centre-to-centre distance treated as a glycosidic link, Ångström.
/// Two linked pyranose centres sit near 5.5 Å; beyond this they are separate
/// molecules that merely pack close.
const MAX_LINK_DISTANCE: f32 = 7.5;

/// One sugar reduced to the point a ribbon passes through and the plane it
/// lies in.
struct Sugar {
    chain: u32,
    centre: Vec3,
    /// Unit normal of the ring's best-fit plane.
    normal: Vec3,
    entity: u32,
    atoms: Vec<u32>,
}

/// Normal of the best-fit plane through a closed ring of points.
///
/// Newell's method sums the cross-products of successive edges, so every ring
/// atom contributes and a pucker — which every real pyranose has — averages out
/// instead of tilting the answer the way picking three atoms would. The result
/// is unnormalized twice the signed area, so its length also reports how
/// planar the ring is; a collapsed ring returns nothing rather than a direction
/// made of rounding error.
fn ring_normal(ring: &[Vec3]) -> Option<Vec3> {
    let mut normal = Vec3::ZERO;
    for (index, current) in ring.iter().enumerate() {
        let next = ring.get((index + 1) % ring.len())?;
        normal += current.cross(*next);
    }
    normal.try_normalize()
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
        output.rebuild_from(Vec::new(), Vec::new(), Vec::new(), Vec::new());
        return;
    }
    let links = link_graph(structure, &sugars);

    let mut points = Vec::new();
    let mut entities = Vec::new();
    let mut normals = Vec::new();
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
            push_sugar(
                &sugars,
                current,
                &mut points,
                &mut entities,
                &mut normals,
                &mut visited,
            );
            while let Some(index) = next {
                if seen(&visited, index) {
                    break;
                }
                push_sugar(
                    &sugars,
                    index,
                    &mut points,
                    &mut entities,
                    &mut normals,
                    &mut visited,
                );
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
                normals.truncate(begin);
            }
        }
    }
    output.rebuild_from(points, entities, normals, ranges);
}

/// Whether a sugar has already been drawn. An index past the end counts as
/// visited, so a bad index ends the run rather than extending it.
fn seen(visited: &[bool], index: usize) -> bool {
    visited.get(index).is_none_or(|drawn| *drawn)
}

/// Appends one chemically oriented sugar to the run being walked.
///
/// The ordered ring names fix which face the normal points toward. Preserving
/// that sign is essential: an alternating 180° ring orientation is structural
/// information that Twister exists to reveal.
fn push_sugar(
    sugars: &[Sugar],
    index: usize,
    points: &mut Vec<Vec3>,
    entities: &mut Vec<u32>,
    normals: &mut Vec<Vec3>,
    visited: &mut [bool],
) {
    let Some(sugar) = sugars.get(index) else {
        return;
    };
    points.push(sugar.centre);
    entities.push(sugar.entity);
    normals.push(sugar.normal);
    if let Some(slot) = visited.get_mut(index) {
        *slot = true;
    }
}

/// Neighbour lists by centre proximity. Explicit glycosidic bonds are the
/// stronger signal, but many deposited entries omit them for branched glycans,
/// and a distance short enough to be a link is unambiguous at sugar scale.
fn link_graph(structure: &pdbiox::Structure, sugars: &[Sugar]) -> Vec<Vec<usize>> {
    let mut links = if structure.data().bonds.is_available() {
        topology_links(structure, sugars)
    } else {
        vec![Vec::new(); sugars.len()]
    };
    complete_by_proximity(sugars, &mut links);
    links
}

fn topology_links(structure: &pdbiox::Structure, sugars: &[Sugar]) -> Vec<Vec<usize>> {
    let Ok(atom_count) = usize::try_from(structure.atom_count()) else {
        return vec![Vec::new(); sugars.len()];
    };
    let mut owner = vec![usize::MAX; atom_count];
    for (sugar, value) in sugars.iter().enumerate() {
        for atom in &value.atoms {
            if let Some(slot) = usize::try_from(*atom)
                .ok()
                .and_then(|index| owner.get_mut(index))
            {
                *slot = sugar;
            }
        }
    }
    let mut links = vec![Vec::new(); sugars.len()];
    for bond in structure.data().bonds.iter() {
        let first = match owner.get(bond.atom_a.as_usize()) {
            Some(value) => *value,
            None => usize::MAX,
        };
        let second = match owner.get(bond.atom_b.as_usize()) {
            Some(value) => *value,
            None => usize::MAX,
        };
        if first == usize::MAX || second == usize::MAX || first == second {
            continue;
        }
        connect(&mut links, first, second);
    }
    links
}

fn complete_by_proximity(sugars: &[Sugar], links: &mut [Vec<usize>]) {
    let mut parents = (0..sugars.len()).collect::<Vec<_>>();
    for (first, neighbours) in links.iter().enumerate() {
        for second in neighbours {
            union(&mut parents, first, *second);
        }
    }
    let mut candidates = Vec::new();
    for (first, left) in sugars.iter().enumerate() {
        for (second, right) in sugars.iter().enumerate().skip(first + 1) {
            if left.chain != right.chain {
                continue;
            }
            let distance = left.centre.distance(right.centre);
            if distance <= MAX_LINK_DISTANCE {
                candidates.push((distance, first, second));
            }
        }
    }
    candidates.sort_by(|left, right| left.0.total_cmp(&right.0));
    for (_, first, second) in candidates {
        if root(&mut parents, first) == root(&mut parents, second) {
            continue;
        }
        connect(links, first, second);
        union(&mut parents, first, second);
    }
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents.get(index).is_some_and(|parent| *parent != index) {
        index = parents[index];
    }
    index
}

fn union(parents: &mut [usize], first: usize, second: usize) {
    let first = root(parents, first);
    let second = root(parents, second);
    if first != second
        && let Some(parent) = parents.get_mut(second)
    {
        *parent = first;
    }
}

fn connect(links: &mut [Vec<usize>], first: usize, second: usize) {
    if let Some(slot) = links.get_mut(first)
        && !slot.contains(&second)
    {
        slot.push(second);
    }
    if let Some(slot) = links.get_mut(second)
        && !slot.contains(&first)
    {
        slot.push(first);
    }
}

fn collect_sugars(structure: &pdbiox::Structure, selection: &AtomSelection) -> Vec<Sugar> {
    let mut sugars = Vec::new();
    for chain in structure.data().chains() {
        let chain_id = chain.index().get();
        for residue in chain.residues() {
            let names = if residue.atom("O5").is_some() {
                &PYRANOSE_RING[..]
            } else {
                &FURANOSE_RING[..]
            };
            let mut ring = Vec::with_capacity(names.len());
            let mut entity = None;
            for name in names {
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
            let atoms = residue.atoms().map(|atom| atom.index().get()).collect();
            // A ring too collapsed to define a plane still has a centre, so it
            // keeps its place on the trace and only gives up its orientation.
            let normal = match ring_normal(&ring) {
                Some(value) => value,
                None => Vec3::ZERO,
            };
            sugars.push(Sugar {
                chain: chain_id,
                centre: sum / count.max(1.0),
                normal,
                entity,
                atoms,
            });
        }
    }
    sugars
}
