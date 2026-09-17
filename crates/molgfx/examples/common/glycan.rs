//! Composing the glycan scene the `Twister` and `PaperChain` pair is for.
//!
//! Both representations describe sugars, and a sugar drawn on its own is a few
//! rings floating in space. Over the polymer they came off they read as what
//! they are, so the scene is built here once rather than in each example.

use molgfx::{Aabb, AtomSelection, RepresentationKind, Scene, Vec3};
use std::error::Error;

/// Whether a residue carries a sugar ring, by the same ring-atom test the
/// glycan geometry uses: a pyranose names its ring oxygen `O5`, a furanose
/// `O4`, and both anchor the ring at `C1`.
fn is_sugar(residue: &pdbiox::ResidueRef<'_>) -> bool {
    residue.atom("O5").is_some() || (residue.atom("O4").is_some() && residue.atom("C1").is_some())
}

/// Ascending atom rows belonging to a sugar residue.
fn sugar_rows(structure: &pdbiox::Structure) -> Vec<u32> {
    let mut rows = Vec::new();
    for residue in structure.data().residues() {
        if !is_sugar(&residue) {
            continue;
        }
        for atom in residue.atoms() {
            rows.push(atom.index().get());
        }
    }
    rows.sort_unstable();
    rows
}

/// Ascending atom rows of the polymer the sugars hang off. A residue counts as
/// polymer when it has an alpha carbon, which is also what the cartoon spline
/// needs to draw it at all.
fn polymer_rows(structure: &pdbiox::Structure, sugars: &[u32]) -> Vec<u32> {
    let mut rows = Vec::new();
    for residue in structure.data().residues() {
        if residue.atom("CA").is_none() {
            continue;
        }
        for atom in residue.atoms() {
            let row = atom.index().get();
            if sugars.binary_search(&row).is_err() {
                rows.push(row);
            }
        }
    }
    rows.sort_unstable();
    rows
}

/// Bounds around the sugars alone, padded so a lone ring still gets a frame
/// with depth rather than one the camera sits inside.
pub fn sugar_bounds(structure: &pdbiox::Structure) -> Aabb {
    let mut points = Vec::new();
    for residue in structure.data().residues() {
        if !is_sugar(&residue) {
            continue;
        }
        for atom in residue.atoms() {
            if let Some(position) = atom.position() {
                points.push(Vec3::from(position));
            }
        }
    }
    let mut bounds = Aabb::from_points(points);
    let center = bounds.center();
    bounds.extend(center + Vec3::splat(3.0));
    bounds.extend(center - Vec3::splat(3.0));
    bounds
}

/// Adds the polymer cartoon and both sugar representations to one scene.
///
/// # Errors
///
/// Propagates whatever the scene reports when a representation cannot be added.
pub fn compose_glycan_scene(
    scene: &mut Scene,
    structure: &pdbiox::Structure,
) -> Result<(), Box<dyn Error>> {
    let sugars = sugar_rows(structure);
    let polymer = polymer_rows(structure, &sugars);
    if !polymer.is_empty() {
        let handle = scene.add_selection(AtomSelection::Sparse(polymer));
        scene.represent(handle, RepresentationKind::Cartoon)?;
    }
    if !sugars.is_empty() {
        let ribbon = scene.add_selection(AtomSelection::Sparse(sugars.clone()));
        scene.represent(ribbon, RepresentationKind::Twister)?;
        let rings = scene.add_selection(AtomSelection::Sparse(sugars));
        scene.represent(rings, RepresentationKind::PaperChain)?;
    }
    Ok(())
}
