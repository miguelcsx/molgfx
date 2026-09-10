//! Cached, allocation-free local bounds for one representation selection.

use crate::RenderError;
use pdviewx_core::{
    AtomSelection, PlacedStructure, Representation, RepresentationKind, RepresentationTarget,
    SurfaceKind,
};
use pdviewx_math::{Aabb, Vec3};

#[cfg(test)]
#[path = "selection_bounds_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct SelectionBoundsState {
    target: RepresentationTarget,
    surface: Option<SurfaceBoundsState>,
    maximum_displacement: u32,
    coordinates: [u64; 2],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct SurfaceBoundsState {
    kind: SurfaceKind,
    radius_scale: u32,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::scene_gpu) struct SelectionBoundsCache {
    cached: Option<(SelectionBoundsState, Aabb)>,
}

impl SelectionBoundsState {
    fn new(placed: &PlacedStructure, representation: &Representation) -> Self {
        let surface = if representation.kind == RepresentationKind::Surface {
            Some(SurfaceBoundsState {
                kind: representation.params.surface_kind,
                radius_scale: effective_radius_scale(representation).to_bits(),
            })
        } else {
            None
        };
        Self {
            target: representation.target,
            surface,
            maximum_displacement: representation
                .visual
                .as_ref()
                .map_or(0.0, |style| style.program().maximum_displacement())
                .to_bits(),
            coordinates: [
                placed.atoms.coords().generation(),
                placed.trajectory_pair_revision(),
            ],
        }
    }
}

impl SelectionBoundsCache {
    pub(in crate::scene_gpu) const fn new() -> Self {
        Self { cached: None }
    }

    /// Resolves at most once per selected coordinate interval. Time-only
    /// trajectory updates reuse the bound enclosing both resident frames.
    pub(in crate::scene_gpu) fn resolve(
        &mut self,
        placed: &PlacedStructure,
        representation: &Representation,
        selection: &AtomSelection,
    ) -> Result<Aabb, RenderError> {
        let state = SelectionBoundsState::new(placed, representation);
        if let Some((cached, bounds)) = self.cached
            && cached == state
        {
            return Ok(bounds);
        }
        let mut bounds = match state.surface {
            Some(surface) => {
                selected_atom_bounds(placed, selection, f32::from_bits(surface.radius_scale))?
            }
            None => placed.render_bvh()?.bounds(),
        };
        let displacement = f32::from_bits(state.maximum_displacement);
        if !bounds.is_empty() && displacement > 0.0 {
            let padding = Vec3::splat(displacement);
            bounds.min -= padding;
            bounds.max += padding;
        }
        self.cached = Some((state, bounds));
        Ok(bounds)
    }
}

fn effective_radius_scale(representation: &Representation) -> f32 {
    if representation.params.surface_kind == SurfaceKind::Gaussian {
        0.0
    } else {
        representation.params.radius_scale.max(0.0)
    }
}

/// Surface grids use selected atoms rather than the whole-structure BVH so a
/// small pocket keeps the target 0.25 Å sampling density.
pub(in crate::scene_gpu) fn selected_atom_bounds(
    placed: &PlacedStructure,
    selection: &AtomSelection,
    radius_scale: f32,
) -> Result<Aabb, RenderError> {
    let positions = placed.atoms.coords().slice();
    let trajectory = placed.trajectory();
    let radii = placed.atoms.radius().values();
    let mut bounds = Aabb::EMPTY;
    selection.for_each(placed.atoms.len(), |row| {
        let index = row as usize;
        let Some(radius) = radii.get(index) else {
            return;
        };
        let radius = *radius * radius_scale;
        if let Some(segment) = trajectory {
            if let Some(position) = segment.start().positions().get(index) {
                bounds.extend_sphere(Vec3::from_array(*position), radius);
            }
            if let Some(position) = segment.end().positions().get(index) {
                bounds.extend_sphere(Vec3::from_array(*position), radius);
            }
        } else if let Some(position) = positions.get(index) {
            bounds.extend_sphere(Vec3::from_array(*position), radius);
        }
    });
    if bounds.is_empty() {
        Ok(placed.render_bvh()?.bounds())
    } else {
        Ok(bounds)
    }
}
