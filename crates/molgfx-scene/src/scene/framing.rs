//! Cameras that frame part of a scene, without changing it.

use super::Scene;
use crate::error::Error;
use crate::selection::Selection;
use molgfx_math::{Aabb, Camera, Vec3};

impl Scene {
    /// World-space bounds of the atom centres a selection picks, over every
    /// placed structure; `None` when it picks none.
    ///
    /// # Errors
    ///
    /// Returns an error when the query does not compile or evaluate.
    pub fn selection_bounds(&self, selection: impl Into<Selection>) -> Result<Option<Aabb>, Error> {
        Ok(self.selection_extent(selection)?.map(|(bounds, _)| bounds))
    }

    /// The bounds of the atom centres a selection picks, and the largest van
    /// der Waals radius among those atoms in world units.
    fn selection_extent(
        &self,
        selection: impl Into<Selection>,
    ) -> Result<Option<(Aabb, f32)>, Error> {
        let selection = selection.into();
        let mut bounds = Aabb::EMPTY;
        let mut largest = 0.0f32;
        let mut any = false;
        for (_, placed) in self.resolved.structures() {
            let rows = placed.source.select(selection.source())?;
            let positions = placed.atoms.coords().slice();
            let radii = placed.atoms.radius().values();
            let scale = [Vec3::X, Vec3::Y, Vec3::Z]
                .map(|axis| placed.model_to_world.transform_vector3(axis).length())
                .into_iter()
                .fold(0.0f32, f32::max);
            rows.for_each(placed.atoms.len(), |row| {
                if let Some(position) = positions.get(row as usize) {
                    bounds.extend(
                        placed
                            .model_to_world
                            .transform_point3(Vec3::from_array(*position)),
                    );
                    largest = largest.max(radii.get(row as usize).copied().unwrap_or(0.0) * scale);
                    any = true;
                }
            });
        }
        Ok(any.then_some((bounds, largest)))
    }

    /// A camera that frames a selection on a target of `aspect` width over
    /// height, leaving the scene as it is.
    ///
    /// The frame holds the atoms as spheres, not only their centres, so a
    /// single atom or a small ligand is not cropped.
    ///
    /// This is the one-call basis of "show me this": the camera a host places
    /// before it draws, or interpolates towards when it animates a move.
    ///
    /// # Errors
    ///
    /// Returns an error when the query is invalid or picks no atom.
    pub fn frame(&self, selection: impl Into<Selection>, aspect: f32) -> Result<Camera, Error> {
        match self.selection_extent(selection)? {
            Some((bounds, radius)) => {
                let padding = Vec3::splat(radius);
                let padded = Aabb {
                    min: bounds.min - padding,
                    max: bounds.max + padding,
                };
                Ok(Camera::framing_aabb(&padded, aspect))
            }
            None => Err(Error::InvalidSpec(
                "the selection picks no atom to frame".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
#[path = "framing_tests.rs"]
mod tests;
