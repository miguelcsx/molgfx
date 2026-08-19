//! Allocation-free local bounds for one representation selection.

use pdviewx_core::{AtomSelection, PlacedStructure};
use pdviewx_math::{Aabb, Vec3};

#[cfg(test)]
#[path = "selection_bounds_tests.rs"]
mod tests;

/// Surface grids use selected atoms rather than the whole-structure BVH so a
/// small pocket keeps the target 0.25 Å sampling density.
pub(super) fn selected_atom_bounds(placed: &PlacedStructure, selection: &AtomSelection) -> Aabb {
    let positions = placed.atoms.coords().slice();
    let radii = placed.atoms.radius().values();
    let mut bounds = Aabb::EMPTY;
    selection.for_each(placed.atoms.len(), |row| {
        let index = row as usize;
        if let (Some(position), Some(radius)) = (positions.get(index), radii.get(index)) {
            bounds.extend_sphere(Vec3::from_array(*position), *radius);
        }
    });
    if bounds.is_empty() {
        placed.render_bvh().bounds()
    } else {
        bounds
    }
}
