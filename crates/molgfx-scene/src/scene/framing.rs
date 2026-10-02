//! Cameras that frame part of a scene, without changing it.

use super::Scene;
use crate::error::Error;
use crate::selection::Selection;
use molgfx_math::{Aabb, Camera, Vec3};

impl Scene {
    /// World-space bounds of the atoms a selection picks, over every placed
    /// structure; `None` when it picks none.
    ///
    /// # Errors
    ///
    /// Returns an error when the query does not compile or evaluate.
    pub fn selection_bounds(&self, selection: impl Into<Selection>) -> Result<Option<Aabb>, Error> {
        let selection = selection.into();
        let mut bounds = Aabb::EMPTY;
        let mut any = false;
        for (_, placed) in self.resolved.structures() {
            let rows = placed.source.select(selection.source())?;
            let positions = placed.atoms.coords().slice();
            rows.for_each(placed.atoms.len(), |row| {
                if let Some(position) = positions.get(row as usize) {
                    bounds.extend(
                        placed
                            .model_to_world
                            .transform_point3(Vec3::from_array(*position)),
                    );
                    any = true;
                }
            });
        }
        Ok(any.then_some(bounds))
    }

    /// A camera that frames a selection on a target of `aspect` width over
    /// height, leaving the scene as it is.
    ///
    /// This is the one-call basis of "show me this": the camera a host places
    /// before it draws, or interpolates towards when it animates a move.
    ///
    /// # Errors
    ///
    /// Returns an error when the query is invalid or picks no atom.
    pub fn frame(&self, selection: impl Into<Selection>, aspect: f32) -> Result<Camera, Error> {
        match self.selection_bounds(selection)? {
            Some(bounds) => Ok(Camera::framing_aabb(&bounds, aspect)),
            None => Err(Error::InvalidSpec(
                "the selection picks no atom to frame".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
#[path = "framing_tests.rs"]
mod tests;
